#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PublicationRole {
    Bootstrap,
    MultiGroupBootstrapA,
    MultiGroupBootstrapB,
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
    MultiGroup,
}

struct PublicationExchange {
    role: PublicationRole,
    acknowledged: bool,
}

struct QueryExchange {
    kind: QueryKind,
    eose: bool,
    closed: bool,
}

struct WireClaims<'a> {
    simple_group: &'a str,
    shared: &'a str,
    unique: String,
    custom: &'a str,
    multi_group_creates: [String; 2],
    custom_signature: &'a str,
    multi_groups: [String; 2],
    queried_multi_group: String,
    metadata_name: String,
    metadata_about: &'static str,
    admin_target: String,
    relay_signer: &'a str,
    author: &'a str,
}

#[allow(
    clippy::too_many_lines,
    reason = "one strict pass owns the complete per-exchange causal proof"
)]
fn verify_one_wire(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    index: usize,
    label: &str,
) -> CanaryResult<u64> {
    let (frames, wire_bytes) = wire_frames(snapshot, label)?;
    let multi_groups: [String; 2] = strings(manifest, "multi_group_ids", 2)?
        .try_into()
        .map_err(|_| CanaryError::new("simple-groups manifest omitted two multi-group ids"))?;
    let claims = WireClaims {
        simple_group: string(manifest, "simple_group_id")?,
        shared: string(manifest, "shared_event_id")?,
        unique: strings(manifest, "unique_event_ids", 2)?[index].clone(),
        custom: string(manifest, "custom_event_id")?,
        multi_group_creates: strings(manifest, "multi_group_create_event_ids", 2)?
            .try_into()
            .map_err(|_| CanaryError::new("simple-groups manifest omitted two group creates"))?,
        custom_signature: string(manifest, "custom_event_signature")?,
        queried_multi_group: multi_groups[index].clone(),
        multi_groups,
        metadata_name: strings(manifest, "metadata_names", 2)?[index].clone(),
        metadata_about: if index == 0 {
            "A-only metadata"
        } else {
            "B-only metadata"
        },
        admin_target: strings(manifest, "admin_targets", 2)?[index].clone(),
        relay_signer: string(manifest, "relay_signer_public_key")?,
        author: string(manifest, "author_public_key")?,
    };
    let mut publication_exchanges =
        std::collections::BTreeMap::<(u64, String), PublicationExchange>::new();
    let mut query_exchanges = std::collections::BTreeMap::<(u64, String), QueryExchange>::new();
    let mut publication_roles = BTreeSet::new();
    let mut bootstrap_id = None;
    let mut content_subscription = None;
    let mut records_subscription = None;
    let mut bootstrap_subscription = None;
    let mut multi_group_subscription = None;
    let mut content_events = BTreeSet::new();
    let mut bootstrap_result = None;
    let mut custom_result = None;
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
                &publication_exchanges,
                &mut query_exchanges,
                &mut content_subscription,
                &mut records_subscription,
                &mut bootstrap_subscription,
                &mut multi_group_subscription,
                bootstrap_id.as_deref(),
            )?,
            ("client_to_relay", "EVENT") => {
                let event = event_at(payload, 1)?;
                event.verify().map_err(error)?;
                if event.pubkey.to_hex() != claims.author {
                    return Err(CanaryError::new(
                        "simple-groups client EVENT escaped its author authority",
                    ));
                }
                let role = publication_role(&event, &claims)?;
                if !matches!(
                    role,
                    PublicationRole::Custom
                        | PublicationRole::MultiGroupBootstrapA
                        | PublicationRole::MultiGroupBootstrapB
                ) && !has_exact_tag(&event, "h", claims.simple_group)
                {
                    return Err(CanaryError::new(
                        "simple-groups client EVENT escaped its h authority",
                    ));
                }
                if !publication_roles.insert(role) {
                    return Err(CanaryError::new(
                        "simple-groups wire repeated a claimed publication role",
                    ));
                }
                if role == PublicationRole::Bootstrap {
                    bootstrap_id = Some(event.id.to_hex());
                }
                if publication_exchanges
                    .insert(
                        (connection, event.id.to_hex()),
                        PublicationExchange {
                            role,
                            acknowledged: false,
                        },
                    )
                    .is_some()
                {
                    return Err(CanaryError::new(
                        "simple-groups wire repeated an exact publication exchange",
                    ));
                }
            }
            ("relay_to_client", "OK") => {
                let acknowledged = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let accepted = payload.get(2).and_then(Value::as_bool) == Some(true);
                let Some(PublicationExchange {
                    acknowledged: was_acknowledged,
                    ..
                }) = publication_exchanges.get_mut(&(connection, acknowledged.to_owned()))
                else {
                    return Err(CanaryError::new(
                        "simple-groups OK was not on its EVENT connection",
                    ));
                };
                if !accepted || *was_acknowledged {
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
                &query_exchanges,
                bootstrap_id.as_deref(),
                &mut content_events,
                &mut bootstrap_result,
                &mut custom_result,
                &mut metadata_winner,
                &mut admin_winner,
            )?,
            ("relay_to_client", "EOSE") => {
                update_query_terminal(payload, connection, &mut query_exchanges, true)?;
            }
            ("client_to_relay", "CLOSE") => {
                update_query_terminal(payload, connection, &mut query_exchanges, false)?;
            }
            _ => {}
        }
    }
    verify_complete_wire(
        &claims,
        &publication_exchanges,
        &query_exchanges,
        &publication_roles,
        bootstrap_id.as_deref(),
        bootstrap_result.as_deref(),
        custom_result.as_deref(),
        &content_events,
        metadata_winner.as_ref(),
        admin_winner.as_ref(),
        &[
            content_subscription,
            records_subscription,
            bootstrap_subscription,
            multi_group_subscription,
        ],
    )?;
    Ok(wire_bytes)
}

#[allow(clippy::too_many_arguments)]
fn verify_req(
    payload: &Value,
    connection: u64,
    claims: &WireClaims<'_>,
    publications: &std::collections::BTreeMap<(u64, String), PublicationExchange>,
    queries: &mut std::collections::BTreeMap<(u64, String), QueryExchange>,
    content_subscription: &mut Option<String>,
    records_subscription: &mut Option<String>,
    bootstrap_subscription: &mut Option<String>,
    multi_group_subscription: &mut Option<String>,
    bootstrap_id: Option<&str>,
) -> CanaryResult<()> {
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let filter = payload.get(2).and_then(Value::as_object);
    let query_kind = if filter
        .is_some_and(|filter| exact_filter(filter, "#h", claims.simple_group, &[9], Some(16)))
    {
        require_acked(
            publications,
            [PublicationRole::Shared, PublicationRole::Unique],
        )?;
        assign_once(content_subscription, subscription, "content REQ")?;
        QueryKind::Content
    } else if filter.is_some_and(|filter| {
        exact_filter(
            filter,
            "#d",
            claims.simple_group,
            &[39000, 39001, 39002, 39003, 39004, 39005],
            None,
        )
    }) {
        require_acked(
            publications,
            [PublicationRole::Metadata, PublicationRole::Admin],
        )?;
        assign_once(records_subscription, subscription, "records REQ")?;
        QueryKind::Records
    } else if filter.is_some_and(|filter| {
        filter.len() == 1
            && bootstrap_id.is_some()
            && filter.get("ids") == Some(&json!([bootstrap_id.expect("checked")]))
    }) {
        require_acked(publications, [PublicationRole::Bootstrap])?;
        assign_once(bootstrap_subscription, subscription, "bootstrap REQ")?;
        QueryKind::Bootstrap
    } else if filter.is_some_and(|filter| {
        exact_filter(
            filter,
            "#h",
            &claims.queried_multi_group,
            &[50_029],
            Some(1),
        )
    }) {
        require_acked(
            publications,
            [
                PublicationRole::MultiGroupBootstrapA,
                PublicationRole::MultiGroupBootstrapB,
                PublicationRole::Custom,
            ],
        )?;
        assign_once(multi_group_subscription, subscription, "multi-group REQ")?;
        QueryKind::MultiGroup
    } else {
        return Err(CanaryError::new(
            "simple-groups wire contained an unclaimed auxiliary REQ",
        ));
    };
    if subscription.is_empty() {
        return Err(CanaryError::new("simple-groups REQ omitted subscription"));
    }
    if queries
        .insert(
            (connection, subscription.to_owned()),
            QueryExchange {
                kind: query_kind,
                eose: false,
                closed: false,
            },
        )
        .is_some()
    {
        return Err(CanaryError::new(
            "simple-groups wire repeated an exact query exchange",
        ));
    }
    Ok(())
}

fn publication_role(event: &Event, claims: &WireClaims<'_>) -> CanaryResult<PublicationRole> {
    let id = event.id.to_hex();
    let kind = event.kind.as_u16();
    let role = if id == claims.shared && kind == 9 {
        PublicationRole::Shared
    } else if id == claims.unique && kind == 9 {
        PublicationRole::Unique
    } else if id == claims.multi_group_creates[0]
        && kind == 9007
        && event.content == "controlled multi-group bootstrap"
        && has_exact_tag(event, "h", &claims.multi_groups[0])
    {
        PublicationRole::MultiGroupBootstrapA
    } else if id == claims.multi_group_creates[1]
        && kind == 9007
        && event.content == "controlled multi-group bootstrap"
        && has_exact_tag(event, "h", &claims.multi_groups[1])
    {
        PublicationRole::MultiGroupBootstrapB
    } else if id == claims.custom
        && kind == 50_029
        && event.sig.to_string() == claims.custom_signature
        && event.content == "one arbitrary event across two exact groups"
        && has_exact_multi_group_tags(event, &claims.multi_groups)
    {
        PublicationRole::Custom
    } else if kind == 9007 && event.content == "controlled group bootstrap" {
        PublicationRole::Bootstrap
    } else if kind == 9002
        && event.content.is_empty()
        && has_exact_command_tags(
            event,
            &[
                ["about", claims.metadata_about, ""],
                ["h", claims.simple_group, ""],
                ["name", &claims.metadata_name, ""],
            ],
        )
    {
        PublicationRole::Metadata
    } else if kind == 9000
        && event.content.is_empty()
        && has_exact_command_tags(
            event,
            &[
                ["h", claims.simple_group, ""],
                ["p", &claims.admin_target, "admin"],
            ],
        )
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
    queries: &std::collections::BTreeMap<(u64, String), QueryExchange>,
    bootstrap_id: Option<&str>,
    content_events: &mut BTreeSet<String>,
    bootstrap_result: &mut Option<String>,
    custom_result: &mut Option<String>,
    metadata_winner: &mut Option<Event>,
    admin_winner: &mut Option<Event>,
) -> CanaryResult<()> {
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let event = event_at(payload, 2)?;
    event.verify().map_err(error)?;
    let Some(QueryExchange { kind, closed, .. }) =
        queries.get(&(connection, subscription.to_owned()))
    else {
        return Err(CanaryError::new(
            "simple-groups response EVENT preceded its REQ",
        ));
    };
    if *closed {
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
                || !has_exact_tag(&event, "h", claims.simple_group)
                || bootstrap_result.replace(event.id.to_hex()).is_some()
            {
                return Err(CanaryError::new(
                    "simple-groups bootstrap query did not return its exact publication",
                ));
            }
        }
        QueryKind::Content => {
            if event.pubkey.to_hex() != claims.author
                || !has_exact_tag(&event, "h", claims.simple_group)
                || event.kind.as_u16() != 9
                || !content_events.insert(event.id.to_hex())
            {
                return Err(CanaryError::new(
                    "simple-groups content result escaped its exact author/group query",
                ));
            }
        }
        QueryKind::Records => {
            if !has_exact_tag(&event, "d", claims.simple_group) {
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
        QueryKind::MultiGroup => {
            if event.id.to_hex() != claims.custom
                || event.sig.to_string() != claims.custom_signature
                || event.pubkey.to_hex() != claims.author
                || event.kind.as_u16() != 50_029
                || event.content != "one arbitrary event across two exact groups"
                || !has_exact_multi_group_tags(&event, &claims.multi_groups)
                || !has_multi_group_tag(&event, &claims.queried_multi_group)
                || custom_result.replace(event.id.to_hex()).is_some()
            {
                return Err(CanaryError::new(
                    "simple-groups multi-group query did not return the exact shared publication",
                ));
            }
        }
    }
    Ok(())
}

fn update_query_terminal(
    payload: &Value,
    connection: u64,
    queries: &mut std::collections::BTreeMap<(u64, String), QueryExchange>,
    eose_frame: bool,
) -> CanaryResult<()> {
    let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
    let Some(QueryExchange { eose, closed, .. }) =
        queries.get_mut(&(connection, subscription.to_owned()))
    else {
        return Err(CanaryError::new(
            "simple-groups query terminal frame preceded its REQ",
        ));
    };
    let invalid = if eose_frame {
        *eose || *closed
    } else {
        !*eose || *closed
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
    publications: &std::collections::BTreeMap<(u64, String), PublicationExchange>,
    roles: [PublicationRole; N],
) -> CanaryResult<()> {
    for expected in roles {
        if !publications.values().any(|state| {
            matches!(
                state,
                PublicationExchange {
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
    publications: &std::collections::BTreeMap<(u64, String), PublicationExchange>,
    queries: &std::collections::BTreeMap<(u64, String), QueryExchange>,
    publication_roles: &BTreeSet<PublicationRole>,
    bootstrap_id: Option<&str>,
    bootstrap_result: Option<&str>,
    custom_result: Option<&str>,
    content_events: &BTreeSet<String>,
    metadata_winner: Option<&Event>,
    admin_winner: Option<&Event>,
    subscriptions: &[Option<String>; 4],
) -> CanaryResult<()> {
    let roles = BTreeSet::from([
        PublicationRole::Bootstrap,
        PublicationRole::MultiGroupBootstrapA,
        PublicationRole::MultiGroupBootstrapB,
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
    if publication_roles != &roles
        || bootstrap_id.is_none()
        || bootstrap_result != bootstrap_id
        || custom_result != Some(claims.custom)
        || content_events != &content
        || !metadata_ok
        || !admin_ok
        || subscriptions.iter().any(Option::is_none)
        || publications.len() != 8
        || publications.values().any(|state| !state.acknowledged)
        || queries.len() != 4
        || queries.values().any(|state| !state.eose || !state.closed)
    {
        return Err(CanaryError::new(
            "simple-groups wire did not derive the complete public flow",
        ));
    }
    Ok(())
}

fn has_multi_group_tag(event: &Event, group: &str) -> bool {
    event.tags.iter().any(|tag| tag.as_slice() == ["h", group])
}

fn has_exact_multi_group_tags(event: &Event, groups: &[String; 2]) -> bool {
    let actual = event
        .tags
        .iter()
        .filter_map(|tag| {
            let values = tag.as_slice();
            (values.len() == 2 && values.first().map(String::as_str) == Some("h"))
                .then(|| values[1].clone())
        })
        .collect::<BTreeSet<_>>();
    actual == groups.iter().cloned().collect()
        && event
            .tags
            .iter()
            .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("h"))
            .count()
            == 2
}
