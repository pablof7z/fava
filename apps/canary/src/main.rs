//! Command-line entry point for the Fava end-to-end canary.

use std::path::PathBuf;

use canary::{
    CroissantNip02Options, FlowOptions, ReconOptions, SmokeOptions, run_croissant_nip02_scenario,
    run_live_scenario, run_m3_live_scenario, run_public_recon, run_publication_scenario,
    run_real_relay_smoke, run_routing_scenario, scenario_registry, verify_croissant_run_pair,
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
        "flow-close-child" => canary::run_flow_close_child(arguments.collect()).await,
        _ => Err(usage()),
    }
}

async fn run_scenario(mut arguments: impl Iterator<Item = String>) -> canary::CanaryResult<()> {
    let scenario = arguments.next().ok_or_else(usage)?;
    if let Some(entry) = canary::blocked(&scenario) {
        return canary::refuse(entry).map(|()| unreachable!("a blocked scenario always refuses"));
    }
    if scenario == "dx-flows" {
        let mut relay_url = None;
        let mut seed = String::from("dx-flows");
        let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(usage)?;
            match flag.as_str() {
                "--relay-url" => relay_url = Some(value),
                "--seed" => seed = value,
                "--runs-dir" => runs_directory = PathBuf::from(value),
                _ => return Err(usage()),
            }
        }
        let run = canary::run_flows_scenario(FlowOptions {
            relay_url: relay_url.ok_or_else(usage)?,
            seed,
            runs_directory,
        })
        .await?;
        println!("passed dx-flows");
        println!("evidence: {}", run.display());
        return Ok(());
    }
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
    let evidence = match scenario.as_str() {
        "multi-relay-dedup-provenance" | "reconnect-generation" => {
            run_m3_live_scenario(&scenario, smoke_options(&mut arguments, "live-m3")?).await?
        }
        "async-route-partial-read" | "explicit-route-bypass" | "fallback-reacts" => {
            run_routing_scenario(&scenario, smoke_options(&mut arguments, "live-m4")?).await?
        }
        "explicit-publish-optimistic"
        | "mixed-relay-outcomes"
        | "cancel-pre-handoff"
        | "crash-after-acceptance" => {
            run_publication_scenario(&scenario, smoke_options(&mut arguments, "publish-m5")?)
                .await?
        }
        "explicit-read-eose" | "explicit-read-live-after-eose" | "explicit-read-cancel" => {
            run_live_scenario(&scenario, smoke_options(&mut arguments, "live-m2")?).await?
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
        "usage: canary list | run <enabled-scenario> [--relay-bin PATH] [--seed SEED] [--runs-dir PATH] | run dx-flows --relay-url URL [--seed SEED] [--runs-dir PATH] | verify-croissant-pair --runs-dir PATH | recon --relay URL [--seed SEED] [--runs-dir PATH]",
    )
    .into()
}
