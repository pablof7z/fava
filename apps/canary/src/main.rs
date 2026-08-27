//! Command-line entry point for the Fava end-to-end canary.

use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod pinned_build_input;
#[cfg(target_os = "linux")]
mod pinned_launcher;
#[cfg(target_os = "linux")]
mod sealed_executable;

use canary::{
    CroissantNip02Options, FlowOptions, PhaseFOptions, ReconOptions, SmokeOptions,
    run_communities_lifecycle, run_crash_recovery, run_croissant_nip02_scenario,
    run_croissant_simple_groups_scenario, run_live_scenario, run_m3_live_scenario,
    run_phase_e_gates, run_public_recon, run_publication_scenario, run_real_relay_smoke,
    run_relay29_lifecycle, run_routing_scenario, scenario_registry, verify_croissant_run_pair,
    verify_croissant_simple_groups_pair,
};

struct CroissantSimpleGroupsCliOptions {
    relay_binary: PathBuf,
    source_checkout: PathBuf,
    fava_build_attestation: PathBuf,
    fava_build_source_manifest: PathBuf,
    scenario_seed: String,
    runs_directory: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("canary failed: {error}");
        std::process::exit(1);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one command dispatcher keeps each CLI grammar adjacent to its exact scenario call"
)]
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
        "launch-croissant-simple-groups" => launch_croissant_simple_groups(arguments).await,
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
            let fava_build_tree_flag = arguments.next().ok_or_else(usage)?;
            let fava_build_tree = arguments.next().ok_or_else(usage)?;
            let fava_build_image_flag = arguments.next().ok_or_else(usage)?;
            let fava_build_image = arguments.next().ok_or_else(usage)?;
            let fava_build_manifest_flag = arguments.next().ok_or_else(usage)?;
            let fava_build_manifest = arguments.next().ok_or_else(usage)?;
            let fava_rust_base_flag = arguments.next().ok_or_else(usage)?;
            let fava_rust_base = arguments.next().ok_or_else(usage)?;
            let fava_executable_flag = arguments.next().ok_or_else(usage)?;
            let fava_executable = arguments.next().ok_or_else(usage)?;
            let fava_subject_image_flag = arguments.next().ok_or_else(usage)?;
            let fava_subject_image = arguments.next().ok_or_else(usage)?;
            let croissant_revision_flag = arguments.next().ok_or_else(usage)?;
            let croissant_revision = arguments.next().ok_or_else(usage)?;
            let croissant_executable_flag = arguments.next().ok_or_else(usage)?;
            let croissant_executable = arguments.next().ok_or_else(usage)?;
            if runs_flag != "--runs-dir"
                || fava_revision_flag != "--expected-fava-revision"
                || fava_tree_flag != "--expected-fava-tree-sha256"
                || fava_build_tree_flag != "--expected-fava-build-tree"
                || fava_build_image_flag != "--expected-fava-build-source-image-sha256"
                || fava_build_manifest_flag != "--expected-fava-build-source-manifest-sha256"
                || fava_rust_base_flag != "--expected-fava-rust-base-image-sha256"
                || fava_executable_flag != "--expected-fava-canary-executable-sha256"
                || fava_subject_image_flag != "--expected-fava-canary-subject-image-sha256"
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
                &fava_build_tree,
                &fava_build_image,
                &fava_build_manifest,
                &fava_rust_base,
                &fava_executable,
                &fava_subject_image,
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
        "flow-close-child" => canary::run_flow_close_child(arguments.collect()).await,
        "phase-e-gates" => {
            let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
            while let Some(flag) = arguments.next() {
                let value = arguments.next().ok_or_else(usage)?;
                match flag.as_str() {
                    "--runs-dir" => runs_directory = PathBuf::from(value),
                    _ => return Err(usage()),
                }
            }
            let outcome = run_phase_e_gates(runs_directory).await?;
            println!("passed phase-e-gates");
            println!("gate2 (ephemeral-restart): {}", outcome.gate2_ephemeral);
            println!("gate3 (persistent-restart): {}", outcome.gate3_persistent);
            println!("gate4 (nip05-negative-cache): {}", outcome.gate4_nip05);
            println!("gate5 (nip11-stale-result): {}", outcome.gate5_nip11);
            Ok(())
        }
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
    if scenario == "croissant-simple-groups-public-flow" {
        let options = simple_groups_options(&mut arguments)?;
        let run_directory = run_croissant_simple_groups_scenario(
            options.relay_binary,
            options.source_checkout,
            options.fava_build_attestation,
            options.fava_build_source_manifest,
            options.scenario_seed,
            options.runs_directory,
        )
        .await?;
        println!("passed croissant-simple-groups-public-flow");
        println!("evidence: {}", run_directory.display());
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
        "phase-f-relay29-lifecycle" => {
            let options = phase_f_options(&mut arguments, &scenario)?;
            let run = run_relay29_lifecycle(&options).await?;
            println!("passed {scenario}");
            println!("evidence: {}", run.display());
            return Ok(());
        }
        "phase-f-communities-lifecycle" => {
            let options = phase_f_options(&mut arguments, &scenario)?;
            let run = run_communities_lifecycle(&options).await?;
            println!("passed {scenario}");
            println!("evidence: {}", run.display());
            return Ok(());
        }
        "phase-f-crash-recovery" => {
            let options = phase_f_options(&mut arguments, &scenario)?;
            let run = run_crash_recovery(&options).await?;
            println!("passed {scenario}");
            println!("evidence: {}", run.display());
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
) -> canary::CanaryResult<CroissantSimpleGroupsCliOptions> {
    let mut relay_binary = None;
    let mut source_checkout = None;
    let mut fava_build_attestation = None;
    let mut fava_build_source_manifest = None;
    let mut scenario_seed = String::from("croissant-simple-groups");
    let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(usage)?;
        match flag.as_str() {
            "--relay-bin" => relay_binary = Some(PathBuf::from(value)),
            "--relay-source" => source_checkout = Some(PathBuf::from(value)),
            "--fava-build-attestation" => fava_build_attestation = Some(PathBuf::from(value)),
            "--fava-build-source-manifest" => {
                fava_build_source_manifest = Some(PathBuf::from(value));
            }
            "--seed" => scenario_seed = value,
            "--runs-dir" => runs_directory = PathBuf::from(value),
            _ => return Err(usage()),
        }
    }
    Ok(CroissantSimpleGroupsCliOptions {
        relay_binary: relay_binary.ok_or_else(usage)?,
        source_checkout: source_checkout.ok_or_else(usage)?,
        fava_build_attestation: fava_build_attestation.ok_or_else(usage)?,
        fava_build_source_manifest: fava_build_source_manifest.ok_or_else(usage)?,
        scenario_seed,
        runs_directory,
    })
}

#[allow(
    clippy::unused_async,
    reason = "Linux descriptor launch awaits the owned child; unsupported hosts fail closed first"
)]
async fn launch_croissant_simple_groups(
    mut arguments: impl Iterator<Item = String>,
) -> canary::CanaryResult<()> {
    let binary_flag = arguments.next().ok_or_else(usage)?;
    let binary = PathBuf::from(arguments.next().ok_or_else(usage)?);
    let source_flag = arguments.next().ok_or_else(usage)?;
    let source = PathBuf::from(arguments.next().ok_or_else(usage)?);
    let attestation_flag = arguments.next().ok_or_else(usage)?;
    let attestation = PathBuf::from(arguments.next().ok_or_else(usage)?);
    let source_manifest_flag = arguments.next().ok_or_else(usage)?;
    let source_manifest = PathBuf::from(arguments.next().ok_or_else(usage)?);
    if binary_flag != "--canary-bin"
        || source_flag != "--fava-source"
        || attestation_flag != "--fava-build-attestation"
        || source_manifest_flag != "--fava-build-source-manifest"
    {
        return Err(usage());
    }
    #[cfg(target_os = "linux")]
    return pinned_launcher::launch(&binary, &source, &attestation, &source_manifest, arguments)
        .await;
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (binary, source, attestation, source_manifest, arguments);
        Err(
            std::io::Error::other("descriptor-pinned Fava execution is unsupported on this host")
                .into(),
        )
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

fn phase_f_options(
    arguments: &mut impl Iterator<Item = String>,
    scenario: &str,
) -> canary::CanaryResult<PhaseFOptions> {
    let mut relay29_binary = PathBuf::from("relay29");
    let mut communities_relay_binary = PathBuf::from("communities-relay");
    let mut seed = scenario.to_owned();
    let mut runs_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runs");
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(usage)?;
        match flag.as_str() {
            "--relay29-bin" => relay29_binary = PathBuf::from(value),
            "--communities-relay-bin" => communities_relay_binary = PathBuf::from(value),
            "--seed" => seed = value,
            "--runs-dir" => runs_directory = PathBuf::from(value),
            _ => return Err(usage()),
        }
    }
    Ok(PhaseFOptions {
        relay29_binary,
        communities_relay_binary,
        seed,
        runs_directory,
    })
}

fn usage() -> canary::CanaryError {
    std::io::Error::other(
        "usage: canary list | launch-croissant-simple-groups --canary-bin PATH --fava-source PATH --fava-build-attestation PATH --fava-build-source-manifest PATH --relay-bin PATH --relay-source PATH [--seed SEED] [--runs-dir PATH] | run <enabled-scenario> --fava-build-attestation-fd PATH --fava-build-source-manifest-fd PATH [--relay-bin PATH] [--relay-source PATH] [--seed SEED] [--runs-dir PATH] | run dx-flows --relay-url URL [--seed SEED] [--runs-dir PATH] | verify-croissant-pair --runs-dir PATH | verify-croissant-simple-groups-pair --runs-dir PATH --expected-fava-revision SHA --expected-fava-tree-sha256 SHA256 --expected-fava-build-tree SHA --expected-fava-build-source-image-sha256 SHA256 --expected-fava-build-source-manifest-sha256 SHA256 --expected-fava-rust-base-image-sha256 SHA256 --expected-fava-canary-executable-sha256 SHA256 --expected-fava-canary-subject-image-sha256 SHA256 --expected-croissant-revision SHA --expected-croissant-executable-sha256 SHA256 | recon --relay URL [--seed SEED] [--runs-dir PATH]",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::simple_groups_options;

    #[test]
    fn producer_options_refuse_circular_expected_digest_input() {
        let mut arguments = [
            "--relay-bin",
            "/relay",
            "--relay-source",
            "/source",
            "--expected-canary-executable-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ]
        .into_iter()
        .map(str::to_owned);
        assert!(simple_groups_options(&mut arguments).is_err());
    }
}
