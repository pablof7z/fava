//! Vocabulary approval governance tests.
//!
//! `vocabulary_gate_requires_all_terms_approved` — the repository gate.
//! It loads `docs/internals/vocabulary.toml` and `docs/internals/approvals.jsonl`
//! and fails until every term carries a valid Nostr kind-9999 event signed by
//! the owner pubkey whose content matches the term's current canonical markdown.
//!
//! **This test is expected to fail** on the current repository because no real
//! owner approvals exist yet.
//!
//! The remaining tests use throwaway-key fixtures and are expected to pass.

use nostr::event::Event;
use std::collections::HashMap;
use std::path::Path;

const OWNER: &str = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52";
const APPROVAL_KIND: u16 = 9999;

/// Prose fields rendered in this fixed order, matching `vocabulary_approval.py`.
const PROSE_FIELDS: &[&str] = &[
    "source",
    "protocol",
    "owner",
    "nearest_nostr",
    "meaning",
    "distinction",
    "counterexample",
    "lifecycle",
    "forcing_requirement",
    "falsifier",
];

/// List fields rendered in this fixed order, matching `vocabulary_approval.py`.
const LIST_FIELDS: &[&str] = &["symbols", "crates", "spec_symbols", "spec_crates"];

fn is_known_field(field: &str) -> bool {
    field == "name" || PROSE_FIELDS.contains(&field) || LIST_FIELDS.contains(&field)
}

/// Render one registry term as the canonical text an approval event must sign.
///
/// Mirrors `vocabulary_approval.py::canonical_markdown`.  Returns `Err` if any
/// field value has a wrong TOML type (fail-closed: no unrendered field survives).
fn canonical_markdown(term: &toml::Table) -> Result<String, String> {
    let name = term
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "term missing string 'name'".to_string())?;

    let mut lines: Vec<String> = vec![format!("# {name}"), String::new()];

    // Prose fields in fixed order.
    for &field in PROSE_FIELDS {
        match term.get(field) {
            None => {}
            Some(toml::Value::String(v)) => {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    lines.push(format!("**{field}**: {trimmed}"));
                    lines.push(String::new());
                }
            }
            Some(other) => {
                return Err(format!(
                    "field '{field}' must be a string, got {}",
                    other.type_str()
                ));
            }
        }
    }

    // List fields in fixed order; items are sorted.
    for &field in LIST_FIELDS {
        match term.get(field) {
            None => {}
            Some(toml::Value::Array(arr)) => {
                if arr.is_empty() {
                    continue;
                }
                let mut items: Vec<String> = Vec::new();
                for (i, v) in arr.iter().enumerate() {
                    match v {
                        toml::Value::String(s) => items.push(s.clone()),
                        other => {
                            return Err(format!(
                                "field '{field}[{i}]' must be a string, got {}",
                                other.type_str()
                            ));
                        }
                    }
                }
                items.sort();
                lines.push(format!("**{field}**:"));
                for item in items {
                    lines.push(format!("- {item}"));
                }
                lines.push(String::new());
            }
            Some(other) => {
                return Err(format!(
                    "field '{field}' must be an array, got {}",
                    other.type_str()
                ));
            }
        }
    }

    // Extra fields: sorted alphabetically; only primitive TOML types allowed.
    let mut extra: Vec<&str> = term
        .keys()
        .map(String::as_str)
        .filter(|&k| !is_known_field(k))
        .collect();
    extra.sort_unstable();
    for field in extra {
        let rendered = match term.get(field).expect("key came from iterator") {
            toml::Value::String(s) => s.clone(),
            toml::Value::Integer(n) => n.to_string(),
            toml::Value::Float(f) => f.to_string(),
            toml::Value::Boolean(b) => b.to_string(),
            other => {
                return Err(format!(
                    "extra field '{field}' has unrenderable type {}",
                    other.type_str()
                ));
            }
        };
        lines.push(format!("**{field}**: {rendered}"));
        lines.push(String::new());
    }

    // Mirror Python's `"\n".join(lines).rstrip() + "\n"`.
    Ok(lines.join("\n").trim_end().to_string() + "\n")
}

/// A cryptographically verified approval event reduced to the fields we need.
struct Approval {
    id: String,
    content: String,
}

/// Parse, cryptographically verify, and index every approval in `text`.
///
/// Duplicate events (same id) are silently dropped.  Competing events
/// (same term name, different id) are recorded as failures.
/// Returns `(approvals_by_name, failures)`.
fn load_and_verify_approvals(text: &str, source: &str) -> (HashMap<String, Approval>, Vec<String>) {
    let mut approvals: HashMap<String, Approval> = HashMap::new();
    let mut failures: Vec<String> = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        if line.trim().is_empty() {
            continue;
        }

        let event: Event = match Event::from_json(line) {
            Ok(ev) => ev,
            Err(e) => {
                failures.push(format!("{source}:{line_no}: parse failed: {e}"));
                continue;
            }
        };

        let id = event.id.to_string();

        if let Err(e) = event.verify() {
            failures.push(format!("{source}:{line_no} id={id}: crypto failed: {e}"));
            continue;
        }

        if event.pubkey.to_hex() != OWNER {
            failures.push(format!(
                "{source}:{line_no} id={id}: pubkey is not the owner: {}",
                event.pubkey.to_hex()
            ));
            continue;
        }

        if event.kind.as_u16() != APPROVAL_KIND {
            failures.push(format!(
                "{source}:{line_no} id={id}: kind must be {APPROVAL_KIND}, got {}",
                event.kind.as_u16()
            ));
            continue;
        }

        let name_tags: Vec<&str> = event
            .tags
            .iter()
            .filter(|t| t.kind() == "name")
            .filter_map(|t| t.content())
            .collect();

        let name = match name_tags.len() {
            1 => name_tags[0].to_string(),
            0 => {
                failures.push(format!("{source}:{line_no} id={id}: no name tag"));
                continue;
            }
            n => {
                failures.push(format!(
                    "{source}:{line_no} id={id}: {n} name tags (must be exactly 1)"
                ));
                continue;
            }
        };

        let content = event.content.clone();

        if let Some(existing) = approvals.get(&name) {
            if existing.id != id {
                failures.push(format!(
                    "{source}:{line_no}: competing approvals for '{name}': {} vs {id}",
                    existing.id
                ));
            }
            // Either competing (failure recorded) or exact duplicate — skip.
            continue;
        }

        approvals.insert(name, Approval { id, content });
    }

    (approvals, failures)
}

// ─── Repository gate ──────────────────────────────────────────────────────────

/// Load the real vocabulary and approval files and demand every term is approved.
///
/// Expected to FAIL until every term has a real owner-signed Nostr event.
#[test]
fn vocabulary_gate_requires_all_terms_approved() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vocab_path = manifest.join("../../docs/internals/vocabulary.toml");
    let approvals_path = manifest.join("../../docs/internals/approvals.jsonl");

    let vocab_text = std::fs::read_to_string(&vocab_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", vocab_path.display()));
    let vocab: toml::Value = vocab_text
        .parse()
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", vocab_path.display()));
    let terms = vocab
        .get("term")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{} must contain a [[term]] array", vocab_path.display()));

    let approvals_text = if approvals_path.exists() {
        std::fs::read_to_string(&approvals_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", approvals_path.display()))
    } else {
        String::new()
    };

    let (approvals, mut failures) = load_and_verify_approvals(&approvals_text, "approvals.jsonl");

    for term_val in terms {
        let term = term_val
            .as_table()
            .unwrap_or_else(|| panic!("each [[term]] entry must be a table"));
        let name = term
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("each term must have a string 'name'"));

        let markdown = match canonical_markdown(term) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name}: canonical_markdown error: {e}"));
                continue;
            }
        };

        match approvals.get(name) {
            None => failures.push(format!("{name}: no signed approval")),
            Some(approval) if approval.content != markdown => {
                failures.push(format!(
                    "{name}: approval content does not match current canonical markdown"
                ));
            }
            Some(_) => {}
        }
    }

    assert!(
        failures.is_empty(),
        "vocabulary approval failures:\n{}",
        failures.join("\n")
    );
}

// ─── Throwaway-key fixture tests (always pass) ────────────────────────────────

/// Kind-9999 approval for the "Event" term, signed with secret scalar = 1.
///
/// Pubkey `79be667e…` is NOT the owner.  Used only for crypto and parity tests.
const THROWAWAY_JSON: &str = r##"{"id":"bbc6bc2bb03fcff13f3b465c8edda0269e51c844c2c1d067c77f02962a4d8ac4","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":1700000000,"kind":9999,"tags":[["name","Event"]],"content":"# Event\n\n**source**: nostr\n\n**protocol**: NIP-01\n\n**owner**: nostr\n\n**meaning**: A signed Nostr event.\n","sig":"f218801e16e03833d7c9d6a8bc179a68e0747c15960fb19487be2326bf687acd3187b1a58a3f6b9a07647e6298d42c5c22a6f8ac1123417ad7e62e17356bd2a2"}"##;

/// `nostr::Event::verify()` accepts the throwaway fixture (id hash + Schnorr).
#[test]
fn throwaway_event_is_cryptographically_valid() {
    let event = Event::from_json(THROWAWAY_JSON).expect("parse throwaway JSON");
    event.verify().expect("throwaway event must pass nostr::Event::verify()");
}

/// The throwaway fixture uses a non-owner key, so the governance gate rejects it.
#[test]
fn throwaway_pubkey_is_not_the_owner() {
    let event = Event::from_json(THROWAWAY_JSON).expect("parse throwaway JSON");
    assert_ne!(
        event.pubkey.to_hex(),
        OWNER,
        "throwaway event pubkey must differ from the owner pubkey"
    );
}

/// Rust `canonical_markdown` output matches the Python-generated fixture content.
#[test]
fn canonical_markdown_matches_event_term_fixture() {
    // The "Event" term as it appears in vocabulary.toml.
    let term_toml = "name = \"Event\"\nsource = \"nostr\"\nprotocol = \"NIP-01\"\nmeaning = \"A signed Nostr event.\"\nowner = \"nostr\"\nsymbols = []\ncrates = []\n";
    let term: toml::Table = toml::from_str(term_toml).expect("parse term TOML");
    let got = canonical_markdown(&term).expect("canonical_markdown must succeed");

    // The expected content is exactly what the throwaway event was signed over.
    let event = Event::from_json(THROWAWAY_JSON).expect("parse throwaway JSON");
    assert_eq!(
        got,
        event.content,
        "Rust canonical_markdown must produce the same text as Python's fixture"
    );
}

/// A valid event (crypto OK, wrong pubkey) is rejected by the governance logic.
#[test]
fn governance_rejects_wrong_pubkey_event() {
    let (approvals, failures) = load_and_verify_approvals(THROWAWAY_JSON, "fixture");
    assert!(
        approvals.is_empty(),
        "wrong-pubkey event must not become an approval"
    );
    assert!(
        failures.iter().any(|f| f.contains("pubkey is not the owner")),
        "expected a pubkey-not-owner failure, got: {failures:?}"
    );
}

// ─── Terminal-name invariant ──────────────────────────────────────────────────

/// Check that `symbols` terminal names and `spec_symbols` values equal the
/// containing term's `name`.  Runs against the real registry; expected to FAIL
/// until every hidden concept is promoted to its own term.
///
/// This check runs independently of the crypto gate so it cannot be bypassed
/// by supplying approval events while hidden concepts remain in the registry.
#[test]
fn vocabulary_terminal_names_match_term_names() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vocab_path = manifest.join("../../docs/internals/vocabulary.toml");
    let vocab_text = std::fs::read_to_string(&vocab_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", vocab_path.display()));
    let vocab: toml::Value = vocab_text
        .parse()
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", vocab_path.display()));
    let terms = vocab
        .get("term")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{} must contain a [[term]] array", vocab_path.display()));

    let mut failures: Vec<String> = Vec::new();

    for term_val in terms {
        let term = term_val
            .as_table()
            .unwrap_or_else(|| panic!("each [[term]] entry must be a table"));
        let name = term
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("each term must have a string 'name'"));

        if let Some(toml::Value::Array(syms)) = term.get("symbols") {
            for sym in syms {
                if let Some(s) = sym.as_str() {
                    let terminal = s.rsplit("::").next().unwrap_or(s);
                    if terminal != name {
                        failures.push(format!(
                            "{name}: symbols '{s}' has terminal name '{terminal}', \
                             hiding a differently named concept under term '{name}'"
                        ));
                    }
                }
            }
        }

        if let Some(toml::Value::Array(spec_syms)) = term.get("spec_symbols") {
            for sym in spec_syms {
                if let Some(s) = sym.as_str() {
                    if s != name {
                        failures.push(format!(
                            "{name}: spec_symbols '{s}' must equal term name '{name}'"
                        ));
                    }
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

/// `ShortfallReason` under `SubscriptionPlanner` rejects: terminal name
/// `ShortfallReason` ≠ term name `SubscriptionPlanner`.
#[test]
fn terminal_name_check_rejects_shortfall_reason_under_subscription_planner() {
    let term_toml = r#"
        name = "SubscriptionPlanner"
        symbols = [
            "fava_subscriptions::SubscriptionPlanner",
            "fava_subscriptions::ShortfallReason",
        ]
    "#;
    let term: toml::Table = toml::from_str(term_toml).expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let syms = term
        .get("symbols")
        .and_then(toml::Value::as_array)
        .unwrap();

    let violations: Vec<&str> = syms
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|s| s.rsplit("::").next().unwrap_or(s) != name)
        .collect();

    assert_eq!(violations, ["fava_subscriptions::ShortfallReason"]);
}

/// `RouterSession` in `spec_symbols` under `Router` rejects: value
/// `RouterSession` ≠ term name `Router`.
#[test]
fn terminal_name_check_rejects_router_session_in_spec_symbols() {
    let term_toml = r#"
        name = "Router"
        spec_symbols = ["Router", "RouterSession"]
    "#;
    let term: toml::Table = toml::from_str(term_toml).expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let spec_syms = term
        .get("spec_symbols")
        .and_then(toml::Value::as_array)
        .unwrap();

    let violations: Vec<&str> = spec_syms
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|s| *s != name)
        .collect();

    assert_eq!(violations, ["RouterSession"]);
}

/// Multiple module paths ending in the same terminal name are all accepted.
#[test]
fn terminal_name_check_accepts_multiple_paths_with_same_terminal_name() {
    let term_toml = r#"
        name = "Query"
        symbols = [
            "fava_query::Query",
            "fava_query_standard::Query",
            "fava_query_testkit::Query",
        ]
    "#;
    let term: toml::Table = toml::from_str(term_toml).expect("parse term TOML");
    let name = term.get("name").and_then(toml::Value::as_str).unwrap();
    let syms = term
        .get("symbols")
        .and_then(toml::Value::as_array)
        .unwrap();

    let violations: Vec<&str> = syms
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|s| s.rsplit("::").next().unwrap_or(s) != name)
        .collect();

    assert!(
        violations.is_empty(),
        "expected no violations, got: {violations:?}"
    );
}
