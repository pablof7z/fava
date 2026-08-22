use std::collections::{BTreeMap, BTreeSet};

use fava_query::{EventRecord, QuerySnapshot};
use fava_state::RelayUrl;
use fava_write::{EventId, EventValue, PublicKey, Timestamp};

use crate::{
    Group, GroupAdmins, GroupMembers, GroupMetadata, GroupParticipants, GroupPins, GroupRoles,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostRecords {
    host: RelayUrl,
    metadata: Option<Selected<GroupMetadata>>,
    admins: Option<Selected<GroupAdmins>>,
    members: Option<Selected<GroupMembers>>,
    roles: Option<Selected<GroupRoles>>,
    participants: Option<Selected<GroupParticipants>>,
    pins: Option<Selected<GroupPins>>,
}

impl HostRecords {
    fn empty(host: RelayUrl) -> Self {
        Self {
            host,
            metadata: None,
            admins: None,
            members: None,
            roles: None,
            participants: None,
            pins: None,
        }
    }

    fn consider(&mut self, id: EventId, created_at: Timestamp, record: ParsedRecord) {
        match record {
            ParsedRecord::Metadata(value) => consider(&mut self.metadata, id, created_at, value),
            ParsedRecord::Admins(value) => consider(&mut self.admins, id, created_at, value),
            ParsedRecord::Members(value) => consider(&mut self.members, id, created_at, value),
            ParsedRecord::Roles(value) => consider(&mut self.roles, id, created_at, value),
            ParsedRecord::Participants(value) => {
                consider(&mut self.participants, id, created_at, value);
            }
            ParsedRecord::Pins(value) => consider(&mut self.pins, id, created_at, value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Selected<T> {
    id: EventId,
    created_at: Timestamp,
    value: T,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParsedRecord {
    Metadata(GroupMetadata),
    Admins(GroupAdmins),
    Members(GroupMembers),
    Roles(GroupRoles),
    Participants(GroupParticipants),
    Pins(GroupPins),
}

impl ParsedRecord {
    fn id(&self) -> &str {
        match self {
            Self::Metadata(value) => value.id(),
            Self::Admins(value) => value.id(),
            Self::Members(value) => value.id(),
            Self::Roles(value) => value.id(),
            Self::Participants(value) => value.id(),
            Self::Pins(value) => value.id(),
        }
    }
}

/// Pure bounded projection of ordinary query results into exact relay-local group truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSnapshot {
    events: Vec<EventRecord>,
    hosts: Vec<HostRecords>,
}

impl GroupSnapshot {
    pub(crate) fn project(group: &Group, snapshot: &QuerySnapshot) -> Self {
        let events = deduplicate_events(snapshot);
        let mut hosts: Vec<_> = group.hosts().map(HostRecords::empty).collect();
        for event in &events {
            let Some(parsed) = parse_record(&event.event, group.id()) else {
                continue;
            };
            let actual_hosts: BTreeSet<_> = event
                .relay_evidence
                .observations()
                .map(|observation| observation.session.relay.clone())
                .collect();
            for host in hosts
                .iter_mut()
                .filter(|host| actual_hosts.contains(&host.host))
            {
                host.consider(event.id(), event.created_at(), parsed.clone());
            }
        }
        Self { events, hosts }
    }

    /// Deduplicated query events in deterministic snapshot order.
    #[must_use]
    pub fn events(&self) -> &[EventRecord] {
        &self.events
    }

    /// Configured hosts in the group's deterministic first-occurrence order.
    pub fn hosts(&self) -> impl Iterator<Item = &RelayUrl> {
        self.hosts.iter().map(|records| &records.host)
    }

    /// Restrict typed record access to one configured host without selecting a winner globally.
    #[must_use]
    pub fn at(&self, host: &RelayUrl) -> Option<Self> {
        self.hosts
            .iter()
            .find(|records| &records.host == host)
            .cloned()
            .map(|records| Self {
                events: self.events.clone(),
                hosts: vec![records],
            })
    }

    /// Complete host-local metadata values in configured host order.
    pub fn metadata(&self) -> impl Iterator<Item = (&RelayUrl, &GroupMetadata)> {
        self.hosts.iter().filter_map(|records| {
            records
                .metadata
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Complete host-local administrator records in configured host order.
    pub fn admin_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupAdmins)> {
        self.hosts.iter().filter_map(|records| {
            records
                .admins
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Complete host-local member records in configured host order.
    pub fn member_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupMembers)> {
        self.hosts.iter().filter_map(|records| {
            records
                .members
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Complete host-local role records in configured host order.
    pub fn role_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupRoles)> {
        self.hosts.iter().filter_map(|records| {
            records
                .roles
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Complete host-local participant records in configured host order.
    pub fn participant_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupParticipants)> {
        self.hosts.iter().filter_map(|records| {
            records
                .participants
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Complete host-local pin records in configured host order.
    pub fn pin_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupPins)> {
        self.hosts.iter().filter_map(|records| {
            records
                .pins
                .as_ref()
                .map(|selected| (&records.host, &selected.value))
        })
    }

    /// Positive administrator entries with their exact serving host.
    pub fn admins(&self) -> impl Iterator<Item = (&RelayUrl, &(PublicKey, Vec<String>))> {
        self.admin_records().flat_map(|(host, record)| {
            record
                .admins()
                .iter()
                .filter_map(move |row| row.as_ref().ok().map(|value| (host, value)))
        })
    }

    /// Positive member entries with their exact serving host.
    pub fn members(&self) -> impl Iterator<Item = (&RelayUrl, &PublicKey)> {
        self.member_records().flat_map(|(host, record)| {
            record
                .members()
                .iter()
                .filter_map(move |row| row.as_ref().ok().map(|value| (host, value)))
        })
    }

    /// Whether complete metadata values disagree across configured hosts.
    #[must_use]
    pub fn metadata_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.metadata.as_ref().map(|selected| &selected.value)),
        )
    }

    /// Whether complete administrator records disagree across configured hosts.
    #[must_use]
    pub fn admins_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.admins.as_ref().map(|selected| &selected.value)),
        )
    }

    /// Whether complete member records disagree across configured hosts.
    #[must_use]
    pub fn members_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.members.as_ref().map(|selected| &selected.value)),
        )
    }

    /// Whether complete role records disagree across configured hosts.
    #[must_use]
    pub fn roles_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.roles.as_ref().map(|selected| &selected.value)),
        )
    }

    /// Whether complete participant records disagree across configured hosts.
    #[must_use]
    pub fn participants_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.participants.as_ref().map(|selected| &selected.value)),
        )
    }

    /// Whether complete pin records disagree across configured hosts.
    #[must_use]
    pub fn pins_differ(&self) -> bool {
        values_differ(
            self.hosts
                .iter()
                .map(|host| host.pins.as_ref().map(|selected| &selected.value)),
        )
    }
}

fn deduplicate_events(snapshot: &QuerySnapshot) -> Vec<EventRecord> {
    let mut positions = BTreeMap::<EventId, usize>::new();
    let mut events: Vec<EventRecord> = Vec::new();
    for event in snapshot.events.iter() {
        if let Some(position) = positions.get(&event.id()).copied() {
            let retained = &mut events[position];
            retained.relay_evidence.merge(&event.relay_evidence);
            if retained.publication.is_none() {
                retained.publication.clone_from(&event.publication);
            }
        } else {
            positions.insert(event.id(), events.len());
            events.push(event.clone());
        }
    }
    events
}

fn parse_record(event: &EventValue, group_id: &str) -> Option<ParsedRecord> {
    let record = match event.kind().as_u16() {
        39_000 => GroupMetadata::from_event(event)
            .ok()
            .map(ParsedRecord::Metadata),
        39_001 => GroupAdmins::from_event(event)
            .ok()
            .map(ParsedRecord::Admins),
        39_002 => GroupMembers::from_event(event)
            .ok()
            .map(ParsedRecord::Members),
        39_003 => GroupRoles::from_event(event).ok().map(ParsedRecord::Roles),
        39_004 => GroupParticipants::from_event(event)
            .ok()
            .map(ParsedRecord::Participants),
        39_005 => GroupPins::from_event(event).ok().map(ParsedRecord::Pins),
        _ => None,
    }?;
    (record.id() == group_id).then_some(record)
}

fn consider<T>(slot: &mut Option<Selected<T>>, id: EventId, created_at: Timestamp, value: T) {
    let is_newer = slot.as_ref().is_none_or(|current| {
        created_at > current.created_at || (created_at == current.created_at && id < current.id)
    });
    if is_newer {
        *slot = Some(Selected {
            id,
            created_at,
            value,
        });
    }
}

fn values_differ<'a, T: PartialEq + 'a>(values: impl Iterator<Item = Option<&'a T>>) -> bool {
    let mut values = values;
    let Some(first) = values.next() else {
        return false;
    };
    values.any(|value| value != first)
}
