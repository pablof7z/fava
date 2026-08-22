//! Command-line entry point for the Fava end-to-end canary.

use std::path::PathBuf;

use canary::{
    CroissantNip02Options, CroissantSimpleGroupsOptions, ReconOptions, SmokeOptions,
    run_automatic_publication_scenario, run_croissant_nip02_scenario,
    run_croissant_simple_groups_scenario, run_grouping_scenario, run_live_scenario,
    run_local_scenario, run_m3_live_scenario, run_public_recon, run_publication_scenario,
    run_real_relay_smoke, run_routing_scenario, run_semantic_write_scenario, scenario_registry,
    verify_croissant_run_pair, verify_croissant_simple_groups_pair,
};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("canary failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> canary::CanaryResult<()> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(usage());
    };
    match command.as_str() {
        "list" => {
            for scenario in scenario_registry()? {
                println!(
                    "{}\t{}\t{}",
                    scenario.id, scenario.milestone, scenario.status
                );
            }
            Ok(())
        }
        "run" => run_scenario(arguments).await,
        "verify-croissant-pair" => {
            let flag = arguments.next().ok_or_else(usage)?;
            let root = arguments.next().ok_or_else(usage)?;
            if flag != "--runs-dir" || arguments.next().is_some() {
                return Err(usage());
            }
            verify_croissant_run_pair(PathBuf::from(root))?;
            println!("verified Croissant NIP-02 pair");
            Ok(())
        }
        "verify-croissant-simple-groups-pair" => {
            let runs_flag = arguments.next().ok_or_else(usage)?;
            let root = arguments.next().ok_or_else(usage)?;
            let fava_revision_flag = arguments.next().ok_or_else(usage)?;
            let fava_revision = arguments.next().ok_or_else(usage)?;
            let fava_tree_flag = arguments.next().ok_or_else(usage)?;
            let fava_tree = arguments.next().ok_or_else(usage)?;
            let fava_executable_flag = arguments.next().ok_or_else(usage)?;
            let fava_executable = arguments.next().ok_or_else(usage)?;
            let croissant_revision_flag = arguments.next().ok_or_else(usage)?;
            let croissant_revision = arguments.next().ok_or_else(usage)?;
            let croissant_executable_flag = arguments.next().ok_or_else(usage)?;
            let croissant_executable = arguments.next().ok_or_else(usage)?;
            if runs_flag != "--runs-dir"
                || fava_revision_flag != "--expected-fava-revision"
                || fava_tree_flag != "--expected-fava-tree-sha256"
                || fava_executable_flag != "--expected-fava-canary-executable-sha256"
                || croissant_revision_flag != "--expected-croissant-revision"
                || croissant_executable_flag != "--expected-croissant-executable-sha256"
                || arguments.next().is_some()
            {
                return Err(usage());
            }
            verify_croissant_simple_groups_pair(
                PathBuf::from(root),
                &fava_revision,
                &fava_tree,
                &fava_executable,
                &croissant_revision,
                &croissant_executable,
            )?;
            println!("verified Croissant simple-groups pair");
            Ok(())
        }
        "recon" => {
            let mut relay_url = None;
            let mut seed = String::from("public-recon");
            let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
            while let Some(flag) = arguments.next() {
                let value = arguments.next().ok_or_else(usage)?;
                match flag.as_str() {
                    "--relay" => relay_url = Some(value),
                    "--seed" => seed = value,
                    "--runs-dir" => runs_directory = PathBuf::from(value),
                    _ => return Err(usage()),
                }
            }
            let outcome = run_public_recon(ReconOptions {
                relay_url: relay_url.ok_or_else(usage)?,
                seed,
                runs_directory,
            })
            .await?;
            println!("recorded public-relay-recon");
            println!("terminal: {}", outcome.terminal);
            println!("frames: {}", outcome.frame_count);
            println!("evidence: {}", outcome.run_directory.display());
            Ok(())
        }
        "crash-child" => canary::run_crash_child(arguments.collect()).await,
        _ => Err(usage()),
    }
}

async fn run_scenario(mut arguments: impl Iterator<Item = String>) -> canary::CanaryResult<()> {
    let scenario = arguments.next().ok_or_else(usage)?;
    if scenario == "croissant-nip02-public-flow" {
        let options = smoke_options(&mut arguments, "croissant-nip02")?;
        let outcome = run_croissant_nip02_scenario(CroissantNip02Options {
            relay_binary: options.relay_binary,
            scenario_seed: options.seed,
            runs_directory: options.runs_directory,
        })
        .await?;
        println!("passed croissant-nip02-public-flow");
        println!("evidence: {}", outcome.run_directory.display());
        return Ok(());
    }
    if scenario == "croissant-simple-groups-public-flow" {
        let options = simple_groups_options(&mut arguments)?;
        let outcome = run_croissant_simple_groups_scenario(options).await?;
        println!("passed croissant-simple-groups-public-flow");
        println!("evidence: {}", outcome.run_directory.display());
        return Ok(());
    }
    if matches!(
        scenario.as_str(),
        "local-source-merge"
            | "local-replaceable-shadow-and-cancel"
            | "local-source-removal"
            | "slow-consumer-latest-state"
    ) {
        let mut seed = String::from("local-m1");
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(usage)?;
            match flag.as_str() {
                "--seed" => seed = value,
                _ => return Err(usage()),
            }
        }
        let event_count = run_local_scenario(&scenario, &seed).await?;
        println!("passed {scenario}");
        println!("events: {event_count}");
        return Ok(());
    }
    let evidence = match scenario.as_str() {
        "multi-relay-dedup-provenance" | "reconnect-generation" => {
            run_m3_live_scenario(&scenario, smoke_options(&mut arguments, "live-m3")?).await?
        }
        "async-route-partial-read" | "explicit-route-bypass" | "fallback-reacts" => {
            run_routing_scenario(&scenario, smoke_options(&mut arguments, "live-m4")?).await?
        }
        "subscription-grouping-equivalence" => {
            run_grouping_scenario(smoke_options(&mut arguments, "grouping-m4")?).await?
        }
        "explicit-publish-optimistic"
        | "mixed-relay-outcomes"
        | "cancel-pre-handoff"
        | "crash-after-acceptance" => {
            run_publication_scenario(&scenario, smoke_options(&mut arguments, "publish-m5")?)
                .await?
        }
        "async-recipient-routing"
        | "hint-routing"
        | "route-preview-parity"
        | "app-relay-versus-fallback-profile" => {
            run_automatic_publication_scenario(
                &scenario,
                smoke_options(&mut arguments, "routing-m6")?,
            )
            .await?
        }
        "explicit-read-eose" | "explicit-read-live-after-eose" | "explicit-read-cancel" => {
            run_live_scenario(&scenario, smoke_options(&mut arguments, "live-m2")?).await?
        }
        "replaceable-edit-first-value"
        | "replaceable-edit-rematerialization"
        | "replaceable-edit-opposing-operations"
        | "protocol-crate-n-plus-one" => {
            run_semantic_write_scenario(&scenario, smoke_options(&mut arguments, "semantic-m7")?)
                .await?
        }
        "lab-real-relay-smoke" => {
            let outcome =
                run_real_relay_smoke(smoke_options(&mut arguments, "local-smoke")?).await?;
            println!("passed lab-real-relay-smoke");
            println!("event: {}", outcome.event_id);
            println!("evidence: {}", outcome.run_directory.display());
            return Ok(());
        }
        _ => {
            return Err(std::io::Error::other(format!(
                "unknown or unimplemented scenario: {scenario}"
            ))
            .into());
        }
    };
    println!("passed {scenario}");
    println!("evidence: {}", evidence.display());
    Ok(())
}

fn simple_groups_options(
    arguments: &mut impl Iterator<Item = String>,
) -> canary::CanaryResult<CroissantSimpleGroupsOptions> {
    let mut relay_binary = None;
    let mut source_checkout = None;
    let mut scenario_seed = String::from("croissant-simple-groups");
    let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
    let mut expected_canary_executable_sha256 = None;
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(usage)?;
        match flag.as_str() {
            "--relay-bin" => relay_binary = Some(PathBuf::from(value)),
            "--relay-source" => source_checkout = Some(PathBuf::from(value)),
            "--seed" => scenario_seed = value,
            "--runs-dir" => runs_directory = PathBuf::from(value),
            "--expected-canary-executable-sha256" => {
                expected_canary_executable_sha256 = Some(value);
            }
            _ => return Err(usage()),
        }
    }
    Ok(CroissantSimpleGroupsOptions {
        relay_binary: relay_binary.ok_or_else(usage)?,
        source_checkout: source_checkout.ok_or_else(usage)?,
        scenario_seed,
        runs_directory,
        expected_canary_executable_sha256: expected_canary_executable_sha256.ok_or_else(usage)?,
    })
}

fn smoke_options(
    arguments: &mut impl Iterator<Item = String>,
    default_seed: &str,
) -> canary::CanaryResult<SmokeOptions> {
    let mut relay_binary = PathBuf::from("nostr-rs-relay");
    let mut seed = default_seed.to_owned();
    let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(usage)?;
        match flag.as_str() {
            "--relay-bin" => relay_binary = PathBuf::from(value),
            "--seed" => seed = value,
            "--runs-dir" => runs_directory = PathBuf::from(value),
            _ => return Err(usage()),
        }
    }
    Ok(SmokeOptions {
        relay_binary,
        seed,
        runs_directory,
    })
}

fn usage() -> canary::CanaryError {
    std::io::Error::other(
        "usage: canary list | run <enabled-scenario> [--relay-bin PATH] [--relay-source PATH] [--seed SEED] [--runs-dir PATH] [--expected-canary-executable-sha256 SHA256] | verify-croissant-pair --runs-dir PATH | verify-croissant-simple-groups-pair --runs-dir PATH --expected-fava-revision SHA --expected-fava-tree-sha256 SHA256 --expected-fava-canary-executable-sha256 SHA256 --expected-croissant-revision SHA --expected-croissant-executable-sha256 SHA256 | recon --relay URL [--seed SEED] [--runs-dir PATH]",
    )
    .into()
}
