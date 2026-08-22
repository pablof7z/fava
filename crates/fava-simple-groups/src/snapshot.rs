use fava_query::{EventRecord, QuerySnapshot};
use fava_state::RelayUrl;
use fava_write::PublicKey;

use crate::{
    Group, GroupAdmins, GroupMembers, GroupMetadata, GroupParticipants, GroupPins, GroupRoles,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostRecords {
    host: RelayUrl,
    metadata: Option<GroupMetadata>,
    admins: Option<GroupAdmins>,
    members: Option<GroupMembers>,
    roles: Option<GroupRoles>,
    participants: Option<GroupParticipants>,
    pins: Option<GroupPins>,
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
}

/// Pure bounded projection of ordinary query results into exact relay-local group truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSnapshot {
    events: Vec<EventRecord>,
    hosts: Vec<HostRecords>,
}

impl GroupSnapshot {
    pub(crate) fn project(group: &Group, snapshot: &QuerySnapshot) -> Self {
        // Compile-complete one-host tracer; the complete projection follows the runtime RED.
        let hosts = group.hosts().take(1).map(HostRecords::empty).collect();
        Self {
            events: snapshot.events.iter().cloned().collect(),
            hosts,
        }
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
                .map(|value| (&records.host, value))
        })
    }

    /// Complete host-local administrator records in configured host order.
    pub fn admin_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupAdmins)> {
        self.hosts
            .iter()
            .filter_map(|records| records.admins.as_ref().map(|value| (&records.host, value)))
    }

    /// Complete host-local member records in configured host order.
    pub fn member_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupMembers)> {
        self.hosts
            .iter()
            .filter_map(|records| records.members.as_ref().map(|value| (&records.host, value)))
    }

    /// Complete host-local role records in configured host order.
    pub fn role_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupRoles)> {
        self.hosts
            .iter()
            .filter_map(|records| records.roles.as_ref().map(|value| (&records.host, value)))
    }

    /// Complete host-local participant records in configured host order.
    pub fn participant_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupParticipants)> {
        self.hosts.iter().filter_map(|records| {
            records
                .participants
                .as_ref()
                .map(|value| (&records.host, value))
        })
    }

    /// Complete host-local pin records in configured host order.
    pub fn pin_records(&self) -> impl Iterator<Item = (&RelayUrl, &GroupPins)> {
        self.hosts
            .iter()
            .filter_map(|records| records.pins.as_ref().map(|value| (&records.host, value)))
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
        false
    }

    /// Whether complete administrator records disagree across configured hosts.
    #[must_use]
    pub fn admins_differ(&self) -> bool {
        false
    }

    /// Whether complete member records disagree across configured hosts.
    #[must_use]
    pub fn members_differ(&self) -> bool {
        false
    }

    /// Whether complete role records disagree across configured hosts.
    #[must_use]
    pub fn roles_differ(&self) -> bool {
        false
    }

    /// Whether complete participant records disagree across configured hosts.
    #[must_use]
    pub fn participants_differ(&self) -> bool {
        false
    }

    /// Whether complete pin records disagree across configured hosts.
    #[must_use]
    pub fn pins_differ(&self) -> bool {
        false
    }
}
