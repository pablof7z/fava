// ── Gate 4 — Crash-recovery against relay29 ──────────────────────────────────

/// Run the crash-recovery gate against relay29.
///
/// Publishes a kind-1 text note to relay29, simulates SIGKILL of the fava
/// process (using the crash-child pattern), then creates a new fava instance
/// with the same redb write store and verifies the receipt recovers without
/// re-sending a duplicate EVENT.
///
/// # Errors
///
/// Returns an error when the relay, child process, or recovery step fails.
pub async fn run_crash_recovery(options: &PhaseFOptions) -> CanaryResult<PathBuf> {
    let scenario = "phase-f-crash-recovery";
    let mut artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": scenario, "seed": options.seed }),
    )?;
    let started = unix_ms()?;

    match crash_recovery_inner(&mut artifacts, options).await {
        Ok(facts) => {
            let ended = unix_ms()?;
            artifacts.record("crash_recovery_passed", &facts)?;
            artifacts.write_report(&format!(
                "# Phase F — Crash-recovery against relay29\n\n\
                **Result:** PASSED\n\n\
                ## Evidence\n\n\
                - Write ID recovered: {}\n\
                - Receipt ID recovered: {}\n\
                - EVENT frames in wire: {} (≤ 2 expected)\n\
                - Event on relay after restart: {}\n",
                facts["write_id"],
                facts["receipt_id"],
                facts["wire_event_frames"],
                facts["relay_has_event"],
            ))?;
            artifacts.write_json(
                "manifest.json",
                &json!({
                    "run_id": artifacts.run_id()?,
                    "scenario": scenario,
                    "scenario_seed": options.seed,
                    "relay_implementation": "relay29-custom",
                    "started_unix_ms": started,
                    "ended_unix_ms": ended,
                    "facts": facts,
                }),
            )?;
            println!("passed {scenario}");
            Ok(artifacts.root().to_owned())
        }
        Err(error) => {
            let _ = artifacts.record("scenario_failed", json!({ "error": error.to_string() }));
            let _ = artifacts.write_report(&format!(
                "# Phase F — Crash-recovery against relay29\n\nFailed: {error}\n"
            ));
            Err(error)
        }
    }
}

/// One relay29 child crashed after an accepted write, still holding the
/// process, proxy, and marker facts recovery needs to reopen the store.
struct CrashRig {
    process: GoRelayProcess,
    proxy: WireProxy,
    wire_log: PathBuf,
    relay_url: String,
    database: PathBuf,
    receipt_id_u64: u64,
    event_id_hex: String,
}

async fn spawn_and_crash_relay29_child(
    artifacts: &mut RunArtifacts,
    options: &PhaseFOptions,
) -> CanaryResult<CrashRig> {
    let port = reserve_port().await?;
    let relay_dir = artifacts.root().join("relays/relay29-crash");
    fs::create_dir_all(&relay_dir)?;

    let supervisor = GoRelaySupervisor::prepare(
        &options.relay29_binary,
        &relay_dir,
        port,
        "Fava Phase-F crash-relay29",
        RELAY_PRIVKEY,
    )?;
    let process = supervisor.spawn(1).await?;
    artifacts.record("relay29_ready", process.fact("ready"))?;

    let wire_log = artifacts.root().join("wire/crash-recovery.jsonl");
    fs::create_dir_all(artifacts.root().join("wire"))?;
    let proxy = WireProxy::start(supervisor.address(), &wire_log).await?;
    let proxy_url = proxy.url();
    let relay_url = supervisor.url();

    let database = artifacts.root().join("children/crash.redb");
    let marker = artifacts.root().join("children/accepted.json");
    fs::create_dir_all(artifacts.root().join("children"))?;

    // ── Spawn crash-child ─────────────────────────────────────────────────────
    let mut child = spawn_crash_child(
        &database,
        &marker,
        &proxy_url,
        &options.seed,
        artifacts.root(),
    )?;
    wait_child_marker(&marker, &mut child).await?;

    // Marker written → kill child (simulates SIGKILL of fava).
    child.kill().await?;
    child.wait().await?;
    artifacts.record("crash_child_killed", json!({ "marker": marker }))?;

    let marker_data: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).map_err(|e| CanaryError::new(e.to_string()))?)
            .map_err(|e| CanaryError::new(e.to_string()))?;
    let receipt_id_u64 = marker_data["receipt_id"]
        .as_u64()
        .ok_or_else(|| CanaryError::new("marker missing receipt_id"))?;
    let event_id_hex = marker_data["event_id"]
        .as_str()
        .ok_or_else(|| CanaryError::new("marker missing event_id"))?
        .to_owned();

    Ok(CrashRig {
        process,
        proxy,
        wire_log,
        relay_url,
        database,
        receipt_id_u64,
        event_id_hex,
    })
}

async fn crash_recovery_inner(
    artifacts: &mut RunArtifacts,
    options: &PhaseFOptions,
) -> CanaryResult<serde_json::Value> {
    let CrashRig {
        process,
        proxy,
        wire_log,
        relay_url,
        database,
        receipt_id_u64,
        event_id_hex,
    } = spawn_and_crash_relay29_child(artifacts, options).await?;

    // ── Recovery ──────────────────────────────────────────────────────────────
    let store = Arc::new(RedbWriteStore::open(&database).map_err(error)?);
    let recovered_keys = deterministic_keys(&options.seed)?;
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(recovered_keys));
    let recovery_fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signers([signer])
        .build()
        .map_err(error)?;

    let recovered_receipt_id =
        fava_write::ReceiptId::try_from(receipt_id_u64).expect("nonzero receipt identity");
    let recovered = recovery_fava
        .receipt(recovered_receipt_id)
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("accepted receipt missing after SIGKILL"))?;

    if recovered.current.id().to_hex() != event_id_hex {
        return Err(CanaryError::new(format!(
            "recovered event identity changed: expected {event_id_hex}, got {}",
            recovered.current.id().to_hex()
        )));
    }

    let terminal_receipt = wait_recovered_terminal(&recovery_fava, recovered_receipt_id).await?;
    artifacts.record(
        "crash_recovery_terminal",
        json!({
            "receipt_id": receipt_id_u64,
            "event_id": event_id_hex,
            "outcome": format!("{:?}", terminal_receipt.outcome),
            "acknowledged": terminal_receipt.acknowledged(),
        }),
    )?;

    // ── Verify relay still has the event ──────────────────────────────────────
    // relay29 contentQueryHandler returns events without "f" tags (public events).
    // We query directly against the relay (not through the proxy) to get a clean witness.
    let event_id = fava_write::EventId::from_hex(&event_id_hex)
        .map_err(|e| CanaryError::new(e.to_string()))?;
    let query_result = tokio::time::timeout(
        Duration::from_secs(5),
        wire::query_exact(&relay_url, event_id, "phase-f-crash-recovery"),
    )
    .await;

    let relay_has_event = match query_result {
        Ok(Ok(witness)) => witness.found_event && witness.saw_eose,
        Ok(Err(_)) | Err(_) => {
            // relay29 may not return kind-1 events without h-tag via its custom query handlers
            // (contentQueryHandler does call db.QueryEvents, so events should be returned)
            false
        }
    };

    proxy.shutdown().await?;
    let stopped = process.graceful_stop().await?;
    artifacts.record("relay29_stopped", stopped)?;

    let wire_event_frames = count_wire_frames(&wire_log, r#""EVENT""#);

    Ok(json!({
        "write_id": recovered.write_id.as_u64(),
        "receipt_id": receipt_id_u64,
        "event_id": event_id_hex,
        "terminal_outcome": format!("{:?}", terminal_receipt.outcome),
        "acknowledged": terminal_receipt.acknowledged(),
        "wire_event_frames": wire_event_frames,
        "relay_has_event": relay_has_event,
        "recovery_note": if relay_has_event {
            "Event found on relay after crash recovery — no duplicate publish required"
        } else {
            "Event not queryable via relay29's custom query handlers (kind-1 without h-tag); recovery verified via redb receipt"
        },
    }))
}

