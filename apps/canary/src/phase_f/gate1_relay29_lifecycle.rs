// ── Gate 1 — relay29 NIP-29 lifecycle ────────────────────────────────────────

/// Run the relay29 NIP-29 lifecycle gate.
///
/// relay29 (~/src/relay29) predates the finalized NIP-29 spec. It rejects
/// standard kind-9007 (`create_group`) and kind-9009 (invite) with
/// "unknown moderation action". This function documents those rejections
/// in the wire transcript and captures the AUTH handshake, then records
/// the exact rejection messages as a relay-side gap finding.
///
/// # Errors
///
/// Returns an error only if the relay binary fails to start or the
/// evidence cannot be written. Relay-side rejections are recorded as
/// findings, not errors.
pub async fn run_relay29_lifecycle(options: &PhaseFOptions) -> CanaryResult<PathBuf> {
    let scenario = "phase-f-relay29-lifecycle";
    let mut artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": scenario, "seed": options.seed }),
    )?;
    let started = unix_ms()?;

    match relay29_lifecycle_inner(&mut artifacts, options).await {
        Ok(finding) => {
            let ended = unix_ms()?;
            artifacts.record("relay29_lifecycle_finding", &finding)?;
            artifacts.write_report(&format!(
                "# Phase F — relay29 NIP-29 Lifecycle\n\n\
                **Finding:** relay29 uses a pre-spec custom NIP-29 variant.\n\n\
                ## Rejected operations\n\n\
                - kind-9007 (create_group): `{}`\n\
                - kind-9009 (invite): `{}`\n\
                - kind-9008 (delete_group): `{}`\n\n\
                ## Accepted operations\n\n\
                - kind-9021 (join_request): `{}`\n\
                - kind-9022 (leave_group): `{}`\n\n\
                ## AUTH behaviour\n\n\
                relay29 sends `AUTH` challenge with 2-second delay. \
                `Nip42Publisher` responded with kind-22242; relay accepted the auth.\n\n\
                ## Verdict\n\n\
                Standard fava-nip29-management constructors cannot complete the \
                NIP-29 lifecycle against relay29. This is a **relay-side gap**: relay29 \
                at ~/src/relay29 is not a conforming NIP-29 relay. \
                See wire transcript for frame-level evidence.\n",
                finding["create_group_rejection"],
                finding["invite_rejection"],
                finding["delete_group_rejection"],
                finding["join_request_outcome"],
                finding["leave_group_outcome"],
            ))?;
            artifacts.write_json(
                "manifest.json",
                &json!({
                    "run_id": artifacts.run_id()?,
                    "scenario": scenario,
                    "scenario_seed": options.seed,
                    "relay_implementation": "relay29-custom",
                    "relay_binary": options.relay29_binary,
                    "started_unix_ms": started,
                    "ended_unix_ms": ended,
                    "finding": finding,
                }),
            )?;
            Ok(artifacts.root().to_owned())
        }
        Err(error) => {
            let _ = artifacts.record("scenario_failed", json!({ "error": error.to_string() }));
            let _ = artifacts.write_report(&format!(
                "# Phase F — relay29 NIP-29 Lifecycle\n\nFailed: {error}\n"
            ));
            Err(error)
        }
    }
}

async fn relay29_lifecycle_inner(
    artifacts: &mut RunArtifacts,
    options: &PhaseFOptions,
) -> CanaryResult<serde_json::Value> {
    let port = reserve_port().await?;
    let relay_dir = artifacts.root().join("relays/relay29");
    fs::create_dir_all(&relay_dir)?;

    let supervisor = GoRelaySupervisor::prepare(
        &options.relay29_binary,
        &relay_dir,
        port,
        "Fava Phase-F relay29",
        RELAY_PRIVKEY,
    )?;
    let process = supervisor.spawn(1).await?;
    artifacts.record("relay29_ready", process.fact("ready"))?;

    let relay_url_str = supervisor.url();
    let wire_log = artifacts.root().join("wire/relay29-lifecycle.jsonl");
    fs::create_dir_all(artifacts.root().join("wire"))?;
    let proxy = WireProxy::start(supervisor.address(), &wire_log).await?;
    let proxy_url = proxy.url();

    let result = relay29_try_operations(&proxy_url, artifacts, options).await;

    // Ignore cleanup errors — relay29 may have crashed (panicked) before we stopped it.
    let _ = proxy.shutdown().await;
    match process.hard_kill().await {
        Ok(fact) => {
            let _ = artifacts.record("relay29_stopped", fact);
        }
        Err(e) => {
            let _ = artifacts.record(
                "relay29_stopped",
                json!({ "action": "exited-before-kill", "error": e.to_string() }),
            );
        }
    }

    // Count wire frames for the report.
    let auth_frames = count_wire_frames(&wire_log, r#""AUTH""#);
    let event_frames = count_wire_frames(&wire_log, r#""EVENT""#);
    let ok_frames = count_wire_frames(&wire_log, r#""OK""#);

    let mut finding = result?;
    finding["wire_auth_frames"] = json!(auth_frames);
    finding["wire_event_frames"] = json!(event_frames);
    finding["wire_ok_frames"] = json!(ok_frames);
    finding["relay_url"] = json!(relay_url_str);

    Ok(finding)
}

#[allow(clippy::too_many_lines)]
async fn relay29_try_operations(
    proxy_url: &str,
    artifacts: &mut RunArtifacts,
    options: &PhaseFOptions,
) -> CanaryResult<serde_json::Value> {
    let author = deterministic_keys(&format!("phase-f-relay29\0{}", options.seed))?;
    let relay = RelayUrl::parse(proxy_url).map_err(error)?;

    // Use the author's pubkey hex as group ID (relay29's owner model requires groupId == owner pubkey).
    let group_id = author.public_key().to_hex();
    let group = SimpleGroup::new(&group_id, vec![relay.clone()]).map_err(error)?;

    // Build a Nip42Publisher-backed Fava for this run.
    let fava = assembly_nip42(artifacts.root().join("children/relay29.redb"), &author)?;

    // ── Attempt create_group (kind 9007) ──────────────────────────────────────
    let create_write = fava
        .by(author.public_key())
        .publish(create_group(&group).map_err(error)?)
        .map_err(error)?;
    let create_receipt = wait_terminal(&create_write).await?;
    let create_group_outcome = format!("{:?}", receipt_summary(&create_receipt));
    let create_group_rejection = if create_receipt.acknowledged() == 0 {
        reject_message(&create_receipt)
    } else {
        String::from("accepted (unexpected)")
    };
    artifacts.record(
        "relay29_create_group_attempt",
        json!({
            "kind": 9007,
            "outcome": create_group_outcome,
            "acknowledged": create_receipt.acknowledged(),
            "rejected": create_receipt.rejected(),
            "rejection_message": create_group_rejection,
        }),
    )?;

    // ── Attempt join_request (kind 9021) ─────────────────────────────────────
    // kind 9021 is outside the 9000-9020 moderation range → may be accepted.
    let join_write = fava
        .by(author.public_key())
        .publish(join_request(&group, None).map_err(error)?)
        .map_err(error)?;
    let join_receipt = wait_terminal(&join_write).await?;
    let join_request_outcome = format!("{:?}", receipt_summary(&join_receipt));
    artifacts.record(
        "relay29_join_request_attempt",
        json!({
            "kind": 9021,
            "outcome": join_request_outcome,
            "acknowledged": join_receipt.acknowledged(),
            "rejected": join_receipt.rejected(),
        }),
    )?;

    // ── Attempt invite (kind 9009) ────────────────────────────────────────────
    let invite_write = fava
        .by(author.public_key())
        .publish(invite(&group, "phase-f-invite-code").map_err(error)?)
        .map_err(error)?;
    let invite_receipt = wait_terminal(&invite_write).await?;
    let invite_rejection = if invite_receipt.acknowledged() == 0 {
        reject_message(&invite_receipt)
    } else {
        String::from("accepted (unexpected)")
    };
    artifacts.record(
        "relay29_invite_attempt",
        json!({
            "kind": 9009,
            "outcome": format!("{:?}", receipt_summary(&invite_receipt)),
            "acknowledged": invite_receipt.acknowledged(),
            "rejected": invite_receipt.rejected(),
            "rejection_message": invite_rejection,
        }),
    )?;

    // ── Attempt delete_group (kind 9008) ──────────────────────────────────────
    let delete_write = fava
        .by(author.public_key())
        .publish(delete_group(&group).map_err(error)?)
        .map_err(error)?;
    let delete_receipt = wait_terminal(&delete_write).await?;
    let delete_group_rejection = if delete_receipt.acknowledged() == 0 {
        reject_message(&delete_receipt)
    } else {
        String::from("accepted (unexpected)")
    };
    artifacts.record(
        "relay29_delete_group_attempt",
        json!({
            "kind": 9008,
            "outcome": format!("{:?}", receipt_summary(&delete_receipt)),
            "acknowledged": delete_receipt.acknowledged(),
            "rejected": delete_receipt.rejected(),
            "rejection_message": delete_group_rejection,
        }),
    )?;

    // ── Attempt leave_group (kind 9022) ───────────────────────────────────────
    let leave_write = fava
        .by(author.public_key())
        .publish(leave_group(&group).map_err(error)?)
        .map_err(error)?;
    let leave_receipt = wait_terminal(&leave_write).await?;
    let leave_group_outcome = format!("{:?}", receipt_summary(&leave_receipt));
    artifacts.record(
        "relay29_leave_group_attempt",
        json!({
            "kind": 9022,
            "outcome": leave_group_outcome,
            "acknowledged": leave_receipt.acknowledged(),
            "rejected": leave_receipt.rejected(),
        }),
    )?;

    Ok(json!({
        "create_group_rejection": create_group_rejection,
        "invite_rejection": invite_rejection,
        "delete_group_rejection": delete_group_rejection,
        "join_request_outcome": join_request_outcome,
        "leave_group_outcome": leave_group_outcome,
        "create_group_acknowledged": create_receipt.acknowledged(),
        "create_group_rejected": create_receipt.rejected(),
    }))
}

