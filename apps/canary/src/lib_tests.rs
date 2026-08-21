use super::{deterministic_keys, has_executor, run_local_scenario, scenario_registry};

#[test]
fn every_enabled_scenario_has_an_executor() {
    let scenarios = scenario_registry().expect("registry parses");
    for scenario in scenarios
        .iter()
        .filter(|scenario| scenario.status == "enabled")
    {
        assert!(
            has_executor(&scenario.id),
            "missing executor for {}",
            scenario.id
        );
    }
}

#[tokio::test]
async fn local_scenarios_pass_through_the_public_facade() {
    for scenario in [
        "local-source-merge",
        "local-replaceable-shadow-and-cancel",
        "local-source-removal",
        "slow-consumer-latest-state",
    ] {
        run_local_scenario(scenario, "m1-test")
            .await
            .expect("local scenario passes");
    }
}

#[test]
fn disposable_identity_is_seed_deterministic() {
    let first = deterministic_keys("seed").expect("identity derives");
    let second = deterministic_keys("seed").expect("identity derives");
    assert_eq!(first.public_key(), second.public_key());
}
