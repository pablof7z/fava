use super::{BLOCKED, blocked, deterministic_keys, has_executor, scenario_registry};

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

#[test]
fn every_blocked_scenario_is_registered_as_blocked() {
    let scenarios = scenario_registry().expect("registry parses");
    for entry in BLOCKED {
        let scenario = scenarios
            .iter()
            .find(|scenario| scenario.id == entry.id)
            .unwrap_or_else(|| panic!("blocked scenario {} is not registered", entry.id));
        assert_eq!(
            scenario.status, "blocked",
            "{} must be registered as blocked, not {}",
            entry.id, scenario.status
        );
        assert!(
            !has_executor(entry.id),
            "{} must have no executor while it is blocked",
            entry.id
        );
    }
}

#[test]
fn no_enabled_scenario_is_also_blocked() {
    for scenario in scenario_registry().expect("registry parses") {
        if scenario.status == "enabled" {
            assert!(
                blocked(&scenario.id).is_none(),
                "{} is both enabled and blocked",
                scenario.id
            );
        }
    }
}

#[test]
fn disposable_identity_is_seed_deterministic() {
    let first = deterministic_keys("seed").expect("identity derives");
    let second = deterministic_keys("seed").expect("identity derives");
    assert_eq!(first.public_key(), second.public_key());
}
