//! Command-line entry point for the Fava end-to-end canary.

use std::path::PathBuf;

use canary::{
    ReconOptions, SmokeOptions, run_live_scenario, run_local_scenario, run_public_recon,
    run_real_relay_smoke, scenario_registry,
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
        "run" => {
            let scenario = arguments.next().ok_or_else(usage)?;
            if matches!(
                scenario.as_str(),
                "local-source-merge"
                    | "local-replaceable-shadow-and-cancel"
                    | "local-source-removal"
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
            if matches!(
                scenario.as_str(),
                "explicit-read-eose" | "explicit-read-live-after-eose" | "explicit-read-cancel"
            ) {
                let evidence =
                    run_live_scenario(&scenario, smoke_options(&mut arguments, "live-m2")?).await?;
                println!("passed {scenario}");
                println!("evidence: {}", evidence.display());
                return Ok(());
            }
            if scenario != "lab-real-relay-smoke" {
                return Err(std::io::Error::other(format!(
                    "unknown or unimplemented scenario: {scenario}"
                ))
                .into());
            }
            let outcome =
                run_real_relay_smoke(smoke_options(&mut arguments, "local-smoke")?).await?;
            println!("passed lab-real-relay-smoke");
            println!("event: {}", outcome.event_id);
            println!("evidence: {}", outcome.run_directory.display());
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
        _ => Err(usage()),
    }
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
        "usage: canary list | run <enabled-scenario> [--relay-bin PATH] [--seed SEED] [--runs-dir PATH] | recon --relay URL [--seed SEED] [--runs-dir PATH]",
    )
    .into()
}
