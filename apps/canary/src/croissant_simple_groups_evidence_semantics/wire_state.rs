#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PublicationRole {
    Bootstrap,
    Metadata,
    Admin,
    Shared,
    Unique,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryKind {
    Content,
    Records,
    Bootstrap,
}

enum ConnectionState {
    Publish {
        event_id: String,
        role: PublicationRole,
        acknowledged: bool,
    },
    Query {
        subscription: String,
        kind: QueryKind,
        eose: bool,
        closed: bool,
    },
}

struct WireClaims<'a> {
    group: &'a str,
    shared: &'a str,
    unique: String,
    custom: &'a str,
    metadata_name: String,
    admin_target: String,
    relay_signer: &'a str,
    author: &'a str,
}

#[allow(
    clippy::too_many_lines,
    reason = "one strict pass owns the complete per-connection causal proof"
)]
fn verify_one_wire(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    index: usize,
    label: &str,
) -> CanaryResult<u64> {
    let (frames, wire_bytes) = wire_frames(snapshot, label)?;
    let claims = WireClaims {
        group: string(manifest, "group_id")?,
        shared: string(manifest, "shared_event_id")?,
        unique: strings(manifest, "unique_event_ids", 2)?[index].clone(),
        custom: string(manifest, "custom_event_id")?,
        metadata_name: strings(manifest, "metadata_names", 2)?[index].clone(),
        admin_target: strings(manifest, "admin_targets", 2)?[index].clone(),
        relay_signer: string(manifest, "relay_signer_public_key")?,
        author: string(manifest, "author_public_key")?,
    };
    let mut connections = std::collections::BTreeMap::<u64, ConnectionState>::new();
    let mut publications = BTreeSet::new();
    let mut bootstrap_id = None;
    let mut content_subscription = None;
    let mut records_subscription = None;
    let mut bootstrap_subscription = None;
    let mut content_events = BTreeSet::new();
    let mut bootstrap_result = None;
    let mut metadata_winner: Option<Event> = None;
    let mut admin_winner: Option<Event> = None;

    let mut expected_sequence = 1_u64;
    for frame in &frames {
        if frame.get("sequence").and_then(Value::as_u64) != Some(expected_sequence) {
            return Err(CanaryError::new(
                "simple-groups wire sequence was not strict and contiguous",
            ));
        }
        expected_sequence = expected_sequence.saturating_add(1);
        let connection = frame
            .get("connection")
            .and_then(Value::as_u64)
            .filter(|value| *value != 0)
            .ok_or_else(|| CanaryError::new("simple-groups wire omitted connection identity"))?;
        let direction = frame
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(payload) = frame.get("decoded") else {
            continue;
        };
        let Some(kind) = payload.get(0).and_then(Value::as_str) else {
            continue;
        };
        match (direction, kind) {
            ("client_to_relay", "REQ") => verify_req(
                payload,
                connection,
                &claims,
                &mut connections,
                &mut content_subscription,
                &mut records_subscription,
                &mut bootstrap_subscription,
                bootstrap_id.as_deref(),
            )?,
            ("client_to_relay", "EVENT") => {
                if connections.contains_key(&connection) {
                    return Err(CanaryError::new(
                        "simple-groups wire reused a connection for a second exchange",
                    ));
                }
                let event = event_at(payload, 1)?;
                event.verify().map_err(error)?;
                if event.pubkey.to_hex() != claims.author
                    || !has_exact_tag(&event, "h", claims.group)
                {
                    return Err(CanaryError::new(
                        "simple-groups client EVENT escaped its author or h authority",
                    ));
                }
                let role = publication_role(&event, &claims)?;
                if !publications.insert(role) {
                    return Err(CanaryError::new(
                        "simple-groups wire repeated a claimed publication role",
                    ));
                }
                if role == PublicationRole::Bootstrap {
                    bootstrap_id = Some(event.id.to_hex());
                }
                connections.insert(
                    connection,
                    ConnectionState::Publish {
                        event_id: event.id.to_hex(),
                        role,
                        acknowledged: false,
                    },
                );
            }
            ("relay_to_client", "OK") => {
                let acknowledged = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let accepted = payload.get(2).and_then(Value::as_bool) == Some(true);
                let Some(ConnectionState::Publish {
                    event_id,
                    acknowledged: was_acknowledged,
                    ..
                }) = connections.get_mut(&connection)
                else {
                    return Err(CanaryError::new(
                        "simple-groups OK was not on its EVENT connection",
                    ));
                };
                if !accepted || *was_acknowledged || acknowledged != event_id {
                    return Err(CanaryError::new(
                        "simple-groups OK did not causally acknowledge its EVENT",
                    ));
                }
                *was_acknowledged = true;
            }
            ("relay_to_client", "EVENT") => verify_response(
                payload,
                connection,
                &claims,
                &connections,
                bootstrap_id.as_deref(),
                &mut content_events,
                &mut bootstrap_result,
                &mut metadata_winner,
                &mut admin_winner,
            )?,
            ("relay_to_client", "EOSE") => {
                update_query_terminal(payload, connection, &mut connections, true)?;
            }
            ("client_to_relay", "CLOSE") => {
                update_query_terminal(payload, connection, &mut connections, false)?;
            }
            _ => {}
        }
    }
    verify_complete_wire(
        &claims,
        &connections,
        &publications,
        bootstrap_id.as_deref(),
        bootstrap_result.as_deref(),
        &content_events,
        metadata_winner.as_ref(),
        admin_winner.as_ref(),
        [
            content_subscription,
            records_subscription,
            bootstrap_subscription,
        ],
    )?;
    Ok(wire_bytes)
}

#[allow(clippy::too_many_arguments)]
fn verify_req(
    payload: &Value,
    connection: u64,
    claims: &WireClaims<'_>,
    connections: &mut std::collections::BTreeMap<u64, ConnectionState>,
    content_subscription: &mut Option<String>,
    records_subscription: &mut Option<String>,
    bootstrap_subscription: &mut Option<String>,
    bootstrap_id: Option<&str>,
) -> CanaryResult<()> {
    if connections.contains_key(&connection) {
        return Err(CanaryError::new(
            "simple-groups wire reused a connection for a second exchange",
        ));
    }
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let filter = payload.get(2).and_then(Value::as_object);
    let query_kind =
        if filter.is_some_and(|filter| exact_filter(filter, "#h", claims.group, &[9], 16)) {
            require_acked(
                connections,
                [PublicationRole::Shared, PublicationRole::Unique],
            )?;
            assign_once(content_subscription, subscription, "content REQ")?;
            QueryKind::Content
        } else if filter.is_some_and(|filter| {
            exact_filter(
                filter,
                "#d",
                claims.group,
                &[39000, 39001, 39002, 39003, 39004, 39005],
                4096,
            )
        }) {
            require_acked(
                connections,
                [PublicationRole::Metadata, PublicationRole::Admin],
            )?;
            assign_once(records_subscription, subscription, "records REQ")?;
            QueryKind::Records
        } else if filter.is_some_and(|filter| {
            filter.len() == 1
                && bootstrap_id.is_some()
                && filter.get("ids") == Some(&json!([bootstrap_id.expect("checked")]))
        }) {
            require_acked(connections, [PublicationRole::Bootstrap])?;
            assign_once(bootstrap_subscription, subscription, "bootstrap REQ")?;
            QueryKind::Bootstrap
        } else {
            return Err(CanaryError::new(
                "simple-groups wire contained an unclaimed auxiliary REQ",
            ));
        };
    if subscription.is_empty() {
        return Err(CanaryError::new("simple-groups REQ omitted subscription"));
    }
    connections.insert(
        connection,
        ConnectionState::Query {
            subscription: subscription.to_owned(),
            kind: query_kind,
            eose: false,
            closed: false,
        },
    );
    Ok(())
}

fn publication_role(event: &Event, claims: &WireClaims<'_>) -> CanaryResult<PublicationRole> {
    let id = event.id.to_hex();
    let kind = event.kind.as_u16();
    let role = if id == claims.shared && kind == 9 {
        PublicationRole::Shared
    } else if id == claims.unique && kind == 9 {
        PublicationRole::Unique
    } else if id == claims.custom && kind == 50_029 {
        PublicationRole::Custom
    } else if kind == 9007 && event.content == "controlled group bootstrap" {
        PublicationRole::Bootstrap
    } else if kind == 9002
        && event.content.is_empty()
        && has_exact_tag(event, "name", &claims.metadata_name)
    {
        PublicationRole::Metadata
    } else if kind == 9000
        && event.content.is_empty()
        && has_exact_admin_tag(event, &claims.admin_target)
    {
        PublicationRole::Admin
    } else {
        return Err(CanaryError::new(
            "simple-groups client EVENT did not match an exact claimed publication",
        ));
    };
    Ok(role)
}

#[allow(clippy::too_many_arguments)]
fn verify_response(
    payload: &Value,
    connection: u64,
    claims: &WireClaims<'_>,
    connections: &std::collections::BTreeMap<u64, ConnectionState>,
    bootstrap_id: Option<&str>,
    content_events: &mut BTreeSet<String>,
    bootstrap_result: &mut Option<String>,
    metadata_winner: &mut Option<Event>,
    admin_winner: &mut Option<Event>,
) -> CanaryResult<()> {
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let event = event_at(payload, 2)?;
    event.verify().map_err(error)?;
    let Some(ConnectionState::Query {
        subscription: expected,
        kind,
        closed,
        ..
    }) = connections.get(&connection)
    else {
        return Err(CanaryError::new(
            "simple-groups response EVENT preceded its REQ",
        ));
    };
    if subscription != expected || *closed {
        return Err(CanaryError::new(
            "simple-groups response EVENT escaped its open REQ",
        ));
    }
    match kind {
        QueryKind::Bootstrap => {
            let expected_id = bootstrap_id.ok_or_else(|| {
                CanaryError::new("simple-groups bootstrap response lacked its publication")
            })?;
            if event.id.to_hex() != expected_id
                || event.pubkey.to_hex() != claims.author
                || event.kind.as_u16() != 9007
                || !has_exact_tag(&event, "h", claims.group)
                || bootstrap_result.replace(event.id.to_hex()).is_some()
            {
                return Err(CanaryError::new(
                    "simple-groups bootstrap query did not return its exact publication",
                ));
            }
        }
        QueryKind::Content => {
            if event.pubkey.to_hex() != claims.author
                || !has_exact_tag(&event, "h", claims.group)
                || event.kind.as_u16() != 9
                || !content_events.insert(event.id.to_hex())
            {
                return Err(CanaryError::new(
                    "simple-groups content result escaped its exact author/group query",
                ));
            }
        }
        QueryKind::Records => {
            if !has_exact_tag(&event, "d", claims.group) {
                return Err(CanaryError::new(
                    "simple-groups record result escaped its exact group query",
                ));
            }
            if event.pubkey.to_hex() == claims.relay_signer {
                match event.kind.as_u16() {
                    39000 => select_current(metadata_winner, event),
                    39001 => select_current(admin_winner, event),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn select_current(current: &mut Option<Event>, candidate: Event) {
    let replace = current.as_ref().is_none_or(|existing| {
        candidate.created_at > existing.created_at
            || (candidate.created_at == existing.created_at && candidate.id < existing.id)
    });
    if replace {
        *current = Some(candidate);
    }
}

fn update_query_terminal(
    payload: &Value,
    connection: u64,
    connections: &mut std::collections::BTreeMap<u64, ConnectionState>,
    eose_frame: bool,
) -> CanaryResult<()> {
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let Some(ConnectionState::Query {
        subscription: expected,
        eose,
        closed,
        ..
    }) = connections.get_mut(&connection)
    else {
        return Err(CanaryError::new(
            "simple-groups query terminal frame preceded its REQ",
        ));
    };
    let invalid = if eose_frame {
        subscription != expected || *eose || *closed
    } else {
        subscription != expected || !*eose || *closed
    };
    if invalid {
        return Err(CanaryError::new(
            "simple-groups query terminal frame was not causal",
        ));
    }
    if eose_frame {
        *eose = true;
    } else {
        *closed = true;
    }
    Ok(())
}

fn require_acked<const N: usize>(
    connections: &std::collections::BTreeMap<u64, ConnectionState>,
    roles: [PublicationRole; N],
) -> CanaryResult<()> {
    for expected in roles {
        if !connections.values().any(|state| {
            matches!(
                state,
                ConnectionState::Publish {
                    role,
                    acknowledged: true,
                    ..
                } if *role == expected
            )
        }) {
            return Err(CanaryError::new(
                "simple-groups query preceded its accepted publication handoff",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_complete_wire(
    claims: &WireClaims<'_>,
    connections: &std::collections::BTreeMap<u64, ConnectionState>,
    publications: &BTreeSet<PublicationRole>,
    bootstrap_id: Option<&str>,
    bootstrap_result: Option<&str>,
    content_events: &BTreeSet<String>,
    metadata_winner: Option<&Event>,
    admin_winner: Option<&Event>,
    subscriptions: [Option<String>; 3],
) -> CanaryResult<()> {
    let roles = BTreeSet::from([
        PublicationRole::Bootstrap,
        PublicationRole::Metadata,
        PublicationRole::Admin,
        PublicationRole::Shared,
        PublicationRole::Unique,
        PublicationRole::Custom,
    ]);
    let content = BTreeSet::from([claims.shared.to_owned(), claims.unique.clone()]);
    let metadata_ok = metadata_winner.is_some_and(|event| {
        has_exact_tag(event, "name", &claims.metadata_name)
            && event.pubkey.to_hex() == claims.relay_signer
    });
    let admin_ok = admin_winner.is_some_and(|event| {
        has_tag_value(event, "p", &claims.admin_target)
            && event.pubkey.to_hex() == claims.relay_signer
    });
    if publications != &roles
        || bootstrap_id.is_none()
        || bootstrap_result != bootstrap_id
        || content_events != &content
        || !metadata_ok
        || !admin_ok
        || subscriptions.iter().any(Option::is_none)
        || connections.len() != 9
        || connections.values().any(|state| match state {
            ConnectionState::Publish { acknowledged, .. } => !acknowledged,
            ConnectionState::Query { eose, closed, .. } => !eose || !closed,
        })
    {
        return Err(CanaryError::new(
            "simple-groups wire did not derive the complete public flow",
        ));
    }
    Ok(())
}

fn has_exact_admin_tag(event: &Event, target: &str) -> bool {
    let matches = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("p"))
        .filter(|tag| tag.as_slice().get(1).map(String::as_str) == Some(target))
        .collect::<Vec<_>>();
    matches.len() == 1 && matches[0].as_slice().get(2).map(String::as_str) == Some("admin")
}
