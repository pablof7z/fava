//! Independent terminal-name vocabulary invariant.

use std::path::Path;

#[test]
fn vocabulary_terminal_names_match_term_names() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vocab_path = manifest.join("../../docs/internals/vocabulary.toml");
    let vocab_text = std::fs::read_to_string(&vocab_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", vocab_path.display()));
    let vocab: toml::Value = vocab_text
        .parse()
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", vocab_path.display()));
    let terms = vocab
        .get("term")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{} must contain a [[term]] array", vocab_path.display()));
    let mut failures = Vec::new();

    for term_value in terms {
        let term = term_value
            .as_table()
            .unwrap_or_else(|| panic!("each [[term]] entry must be a table"));
        let name = term
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("each term must have a string 'name'"));
        if let Some(toml::Value::Array(symbols)) = term.get("symbols") {
            for symbol in symbols.iter().filter_map(toml::Value::as_str) {
                let terminal = symbol.rsplit("::").next().unwrap_or(symbol);
                if terminal != name {
                    failures.push(format!(
                        "{name}: symbols '{symbol}' has terminal name '{terminal}', \
                         hiding a differently named concept under term '{name}'"
                    ));
                }
            }
        }
        if let Some(toml::Value::Array(symbols)) = term.get("spec_symbols") {
            for symbol in symbols.iter().filter_map(toml::Value::as_str) {
                if symbol != name {
                    failures.push(format!(
                        "{name}: spec_symbols '{symbol}' must equal term name '{name}'"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "vocabulary terminal name violations ({} total):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn rejects_shortfall_reason_under_subscription_planner() {
    let term: toml::Table = toml::from_str(
        r#"
        name = "SubscriptionPlanner"
        symbols = [
            "fava_subscriptions::SubscriptionPlanner",
            "fava_subscriptions::ShortfallReason",
        ]
        "#,
    )
    .expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let violations: Vec<&str> = term["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|symbol| symbol.rsplit("::").next().unwrap_or(symbol) != name)
        .collect();
    assert_eq!(violations, ["fava_subscriptions::ShortfallReason"]);
}

#[test]
fn rejects_router_session_in_spec_symbols() {
    let term: toml::Table = toml::from_str(
        r#"
        name = "Router"
        spec_symbols = ["Router", "RouterSession"]
        "#,
    )
    .expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let violations: Vec<&str> = term["spec_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|symbol| *symbol != name)
        .collect();
    assert_eq!(violations, ["RouterSession"]);
}

#[test]
fn accepts_multiple_paths_with_same_terminal_name() {
    let term: toml::Table = toml::from_str(
        r#"
        name = "Query"
        symbols = [
            "fava_query::Query",
            "fava_query_standard::Query",
            "fava_query_testkit::Query",
        ]
        "#,
    )
    .expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let violations: Vec<&str> = term["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|symbol| symbol.rsplit("::").next().unwrap_or(symbol) != name)
        .collect();
    assert!(
        violations.is_empty(),
        "unexpected violations: {violations:?}"
    );
}
