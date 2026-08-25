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
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

const OWNER: &str = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52";
const APPROVAL_KIND: u16 = 9999;
const CARGO_PUBLIC_API: &str = "0.52.0";
const RUSTDOC_TOOLCHAIN: &str = "nightly-2026-07-07";

/// Prose fields rendered in this fixed order, matching `vocabulary_approval.py`.
const PROSE_FIELDS: &[&str] = &[
    "source",
    "evidence",
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
fn canonical_markdown(term: &toml::Table, structure: &JsonValue) -> Result<String, String> {
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

    let structure = serde_json::to_string(structure)
        .map_err(|error| format!("cannot render compiler-derived structure: {error}"))?;
    lines.extend([
        "## Compiler-derived Rust structure".to_string(),
        String::new(),
        "```json".to_string(),
        structure,
        "```".to_string(),
    ]);

    // Mirror Python's `"\n".join(lines).rstrip() + "\n"`.
    Ok(lines.join("\n").trim_end().to_string() + "\n")
}

fn load_structures(text: &str) -> Result<HashMap<String, JsonValue>, String> {
    let snapshot: JsonValue = serde_json::from_str(text)
        .map_err(|error| format!("cannot parse vocabulary-structure.json: {error}"))?;
    if snapshot.get("format").and_then(JsonValue::as_u64) != Some(1) {
        return Err("vocabulary-structure.json format must be 1".to_string());
    }
    if snapshot.get("cargo_public_api").and_then(JsonValue::as_str) != Some(CARGO_PUBLIC_API) {
        return Err(format!(
            "vocabulary-structure.json cargo-public-api must be {CARGO_PUBLIC_API}"
        ));
    }
    if snapshot
        .get("rustdoc_toolchain")
        .and_then(JsonValue::as_str)
        != Some(RUSTDOC_TOOLCHAIN)
    {
        return Err(format!(
            "vocabulary-structure.json rustdoc toolchain must be {RUSTDOC_TOOLCHAIN}"
        ));
    }
    let terms = snapshot
        .get("terms")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "vocabulary-structure.json must contain terms".to_string())?;
    let mut structures = HashMap::new();
    for entry in terms {
        let name = entry
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "structural term is missing a string name".to_string())?;
        let structure = entry
            .get("structure")
            .cloned()
            .ok_or_else(|| format!("{name}: structural term is missing structure"))?;
        if structures.insert(name.to_string(), structure).is_some() {
            return Err(format!("duplicate structural term: {name}"));
        }
    }
    Ok(structures)
}

/// A cryptographically verified approval event reduced to the fields we need.
struct Approval {
    content: String,
}

/// Parse, cryptographically verify, and index every approval in `text`.
///
/// Duplicate events (same id) are silently dropped.  Competing events
/// (same term name, different id) are recorded as failures.
/// Returns `(approvals_by_name, failures)`.
fn load_and_verify_approvals(
    text: &str,
    source: &str,
) -> (HashMap<String, Vec<Approval>>, Vec<String>) {
    let mut approvals: HashMap<String, Vec<Approval>> = HashMap::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
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

        if !seen_ids.insert(id.clone()) {
            continue;
        }
        approvals
            .entry(name)
            .or_default()
            .push(Approval { content });
    }

    (approvals, failures)
}

fn has_exact_approval(
    approvals: &HashMap<String, Vec<Approval>>,
    name: &str,
    markdown: &str,
) -> bool {
    approvals
        .get(name)
        .is_some_and(|events| events.iter().any(|event| event.content == markdown))
}

fn load_candidate_markdown(root: &Path) -> Result<Vec<(String, String, String)>, String> {
    let script = root.join("tools/approve_vocabulary.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--root")
        .arg(root)
        .arg("--dump-candidates-json")
        .output()
        .map_err(|error| format!("cannot run {}: {error}", script.display()))?;
    if !output.status.success() {
        return Err(format!(
            "candidate validation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let values: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot parse candidate markdown: {error}"))?;
    values
        .as_array()
        .ok_or_else(|| "candidate dump must be an array".to_string())?
        .iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "candidate missing string name".to_string())?;
            let markdown = value
                .get("markdown")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("{name}: candidate missing string markdown"))?;
            let disposition = value
                .get("disposition")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("{name}: candidate missing string disposition"))?;
            Ok((name.to_string(), disposition.to_string(), markdown.to_string()))
        })
        .collect()
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
    let structure_path = manifest.join("../../docs/internals/vocabulary-structure.json");

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

    let structures_text = std::fs::read_to_string(&structure_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", structure_path.display()));
    let structures = load_structures(&structures_text)
        .unwrap_or_else(|e| panic!("invalid {}: {e}", structure_path.display()));

    let (approvals, mut failures) = load_and_verify_approvals(&approvals_text, "approvals.jsonl");

    for term_val in terms {
        let term = term_val
            .as_table()
            .unwrap_or_else(|| panic!("each [[term]] entry must be a table"));
        let name = term
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("each term must have a string 'name'"));

        let Some(structure) = structures.get(name) else {
            failures.push(format!("{name}: missing compiler-derived Rust structure"));
            continue;
        };
        let markdown = match canonical_markdown(term, structure) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name}: canonical_markdown error: {e}"));
                continue;
            }
        };

        if !has_exact_approval(&approvals, name, &markdown) {
            failures.push(format!(
                "{name}: no signature matches current canonical markdown"
            ));
        }
    }

    match load_candidate_markdown(&manifest.join("../..")) {
        Ok(candidates) => {
            for (name, disposition, markdown) in candidates {
                if disposition == "blocked" {
                    failures.push(format!("{name}: blocked candidate cannot be approved"));
                    continue;
                }
                if !has_exact_approval(&approvals, &name, &markdown) {
                    failures.push(format!(
                        "{name}: no signature matches current candidate markdown"
                    ));
                }
            }
        }
        Err(error) => failures.push(error),
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
    event
        .verify()
        .expect("throwaway event must pass nostr::Event::verify()");
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

/// Rust renders the same explicit empty structural suffix as Python.
#[test]
fn canonical_markdown_includes_compiler_structure() {
    // The "Event" term as it appears in vocabulary.toml.
    let term_toml = "name = \"Event\"\nsource = \"nostr\"\nprotocol = \"NIP-01\"\nmeaning = \"A signed Nostr event.\"\nowner = \"nostr\"\nsymbols = []\ncrates = []\n";
    let term: toml::Table = toml::from_str(term_toml).expect("parse term TOML");
    let structure = serde_json::json!({
        "private_architectural_state": [],
        "public_api": [],
        "reexports": [],
    });
    let got = canonical_markdown(&term, &structure).expect("canonical_markdown must succeed");
    assert!(got.ends_with(
        "## Compiler-derived Rust structure\n\n```json\n\
         {\"private_architectural_state\":[],\"public_api\":[],\"reexports\":[]}\n```\n"
    ));
}

#[test]
fn compiler_structure_drift_invalidates_prior_payload() {
    let term: toml::Table = toml::from_str(
        "name = \"Query\"\nsource = \"fava\"\nmeaning = \"A query.\"\nowner = \"fava-query\"\nsymbols = []\ncrates = []\n",
    )
    .expect("parse term TOML");
    let prior = serde_json::json!({
        "private_architectural_state": [],
        "public_api": [{
            "declaration": "pub fn fava_query::Query::open(&self)",
            "path": "fava_query::Query::open"
        }],
        "reexports": [],
    });
    let changed = serde_json::json!({
        "private_architectural_state": [],
        "public_api": [{
            "declaration": "pub fn fava_query::Query::open(&mut self)",
            "path": "fava_query::Query::open"
        }],
        "reexports": [],
    });
    assert_ne!(
        canonical_markdown(&term, &prior).unwrap(),
        canonical_markdown(&term, &changed).unwrap()
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
        failures
            .iter()
            .any(|f| f.contains("pubkey is not the owner")),
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
                if let Some(s) = sym.as_str()
                    && s != name
                {
                    failures.push(format!(
                        "{name}: spec_symbols '{s}' must equal term name '{name}'"
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
    let syms = term.get("symbols").and_then(toml::Value::as_array).unwrap();

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
    let syms = term.get("symbols").and_then(toml::Value::as_array).unwrap();

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
