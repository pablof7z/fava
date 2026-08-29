//! Private E2E support behavioral coverage.

use std::io::{Cursor, Read as _};

use std::sync::Arc;

use e2e_support::{
    CommandResult, E2eSession, InputMode, Limits, OutputFormat, ResultValue, ShellError,
};
use fava::Fava;
use fava_query_standard::StandardQueryEvaluator;
use fava_write_store_memory::MemoryWriteStore;

fn session() -> E2eSession {
    E2eSession::new(
        Limits::standard(),
        Fava::builder()
            .event_cache_ephemeral()
            .write_store(Arc::new(MemoryWriteStore::default()))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .build()
            .unwrap(),
    )
}

fn selected_session() -> E2eSession {
    let mut session = session();
    session
        .execute_line("account new alice", |_, _| unreachable!())
        .unwrap();
    session
}

#[test]
fn interactive_and_scripted_execution_share_aliases_selection_capture_and_jsonl() {
    let mut session = selected_session();
    let result = session
        .execute_line(
            "relay add group ws://127.0.0.1:18101",
            |_, _| unreachable!(),
        )
        .unwrap();
    assert_eq!(result.kind(), "relay-added");

    session
        .execute_line("account switch alice", |_, _| unreachable!())
        .unwrap();
    let created = session
        .execute_line("group create room group", |session, words| {
            assert_eq!(session.selected_account().unwrap().alias(), "alice");
            assert_eq!(
                session.relay(&words[3]).unwrap().as_str(),
                "ws://127.0.0.1:18101"
            );
            CommandResult::success("group-created", "created room")
                .with_field("group", words[2].clone())?
                .with_field("relay", words[3].clone())
        })
        .unwrap();
    assert_eq!(created.field("group"), Some(&ResultValue::text("room")));

    session
        .execute_line("capture group-name group", |_, _| unreachable!())
        .unwrap();
    let event = session
        .execute_line("group event publish --kind 12345 hello", |_, words| {
            assert_eq!(
                words,
                ["group", "event", "publish", "--kind", "12345", "hello"]
            );
            Ok(CommandResult::success("group-event-published", "published"))
        })
        .unwrap();
    assert_eq!(event.kind(), "group-event-published");

    let dump = session.execute_line("dump", |_, _| unreachable!()).unwrap();
    assert_eq!(dump.kind(), "dump");
    assert!(matches!(
        dump.field("captures"),
        Some(ResultValue::Array(captures)) if captures == &[ResultValue::text("group-name")]
    ));

    let json = OutputFormat::JsonLines.render(&created).unwrap();
    assert!(json.contains("\"kind\":\"group-created\""));
    assert!(json.ends_with('\n'));
}

#[test]
fn shared_account_and_relay_commands_keep_selection_and_retention_coherent() {
    let mut session = session();
    let created = session
        .execute_line("account new alice", |_, _| unreachable!())
        .unwrap();
    assert_eq!(created.kind(), "account-created");
    assert_eq!(session.selected_account_alias(), Some("alice"));
    assert_eq!(
        session
            .execute_line("account list", |_, _| unreachable!())
            .unwrap()
            .field("accounts"),
        Some(&ResultValue::array([ResultValue::text("alice")]))
    );
    session
        .execute_line(
            "relay add group ws://127.0.0.1:18101",
            |_, _| unreachable!(),
        )
        .unwrap();
    assert_eq!(
        session
            .execute_line("relay list", |_, _| unreachable!())
            .unwrap()
            .field("relay_aliases"),
        Some(&ResultValue::array([ResultValue::text("group")]))
    );
    assert_eq!(
        session
            .execute_line("relay list", |_, _| unreachable!())
            .unwrap()
            .field("relay_urls"),
        Some(&ResultValue::array([ResultValue::text(
            "ws://127.0.0.1:18101"
        )]))
    );
    session
        .execute_line("relay remove group", |_, _| unreachable!())
        .unwrap();
    session
        .execute_line("account remove alice", |_, _| unreachable!())
        .unwrap();
    assert_eq!(session.selected_account_alias(), None);
    assert!(matches!(
        session.execute_line_with_prompt(
            "account import",
            InputMode::Script,
            |_| Err(ShellError::NonInteractivePrompt),
            |_, _| unreachable!(),
        ),
        Err(ShellError::NonInteractivePrompt)
    ));
}

#[test]
fn account_import_with_inline_nsec_works_in_script_mode() {
    use nostr::key::Keys;
    use nostr::nips::nip19::ToBech32;
    let mut session = session();
    let nsec = Keys::generate().secret_key().to_bech32().unwrap();
    let result = session
        .execute_line(
            &format!("account import imported {nsec}"),
            |_, _| unreachable!(),
        )
        .unwrap();
    assert_eq!(result.kind(), "account-imported");
    assert_eq!(session.selected_account_alias(), Some("imported"));
}

#[test]
fn result_values_are_json_typed_and_only_scalars_are_capturable() {
    let mut session = session();
    let result = CommandResult::success("typed", "typed fields")
        .with_field("count", 3usize)
        .unwrap()
        .with_field("settled", true)
        .unwrap()
        .with_field("ids", ResultValue::array([ResultValue::text("one")]))
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&OutputFormat::JsonLines.render(&result).unwrap()).unwrap();
    assert!(value["fields"]["count"].is_u64());
    assert!(value["fields"]["settled"].is_boolean());
    assert!(value["fields"]["ids"].is_array());
    assert_eq!(
        ResultValue::text(format!("relay echoed {}", "a".repeat(64))),
        ResultValue::text(format!("relay echoed {}", "a".repeat(64)))
    );

    session
        .execute_line("group list", |_, _| Ok(result.clone()))
        .unwrap();
    assert!(matches!(
        session.execute_line("capture ids ids", |_, _| unreachable!()),
        Err(ShellError::NonScalarResultField { name }) if name == "ids"
    ));
    session
        .execute_line("group list", |_, _| {
            CommandResult::success("typed", "scalar").with_field("count", 3usize)
        })
        .unwrap();
    session
        .execute_line("capture count count", |_, _| unreachable!())
        .unwrap();
    assert!(matches!(
        session.execute_line("group list", |_, _| {
            CommandResult::success("typed", "nested").with_field(
                "nested",
                ResultValue::array([ResultValue::array([ResultValue::text("no")])]),
            )
        }),
        Err(ShellError::NestedResultArray)
    ));
}

#[test]
fn bounds_refuse_before_retaining_external_input() {
    let mut session = E2eSession::new(
        Limits::new(1, 1, 1, 8, 256, 64, 3, 8, 32).unwrap(),
        Fava::builder()
            .event_cache_ephemeral()
            .write_store(Arc::new(MemoryWriteStore::default()))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .build()
            .unwrap(),
    );
    session
        .execute_line("account new alice", |_, _| unreachable!())
        .unwrap();
    session
        .execute_line("relay add one ws://127.0.0.1:18101", |_, _| unreachable!())
        .unwrap();
    let error = session
        .execute_line("relay add two ws://127.0.0.1:18102", |_, _| unreachable!())
        .unwrap_err();
    assert!(matches!(
        error,
        ShellError::Limit {
            what: "relay aliases",
            ..
        }
    ));

    let error = session
        .execute_line("capture saved group", |_, _| unreachable!())
        .unwrap_err();
    assert!(matches!(
        error,
        ShellError::MissingResultField { name } if name == "group"
    ));

    let error = session
        .execute_line(
            "group event publish --kind ${missing}",
            |_, _| unreachable!(),
        )
        .unwrap_err();
    assert!(matches!(error, ShellError::UnknownCapture { .. }));
}

#[test]
fn result_field_count_is_bounded_before_last_result_retention() {
    let mut session = E2eSession::new(
        Limits::new(1, 1, 1, 8, 256, 64, 1, 8, 32).unwrap(),
        Fava::builder()
            .event_cache_ephemeral()
            .write_store(Arc::new(MemoryWriteStore::default()))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .build()
            .unwrap(),
    );
    let error = session
        .execute_line("group list", |_, _| {
            CommandResult::success("group-list", "two fields")
                .with_field("groups", "room")?
                .with_field("selected_group", "room")
        })
        .unwrap_err();
    assert!(matches!(
        error,
        ShellError::Limit {
            what: "result fields",
            maximum: 1,
        }
    ));
    let error = session
        .execute_line("capture saved groups", |_, _| unreachable!())
        .unwrap_err();
    assert!(matches!(error, ShellError::MissingResultField { .. }));
}

#[test]
fn required_interactive_values_prompt_without_history_and_replays_refuse() {
    let session = session();
    let mut input = Cursor::new("12345\n");
    let mut output = Vec::new();
    assert_eq!(
        session
            .prompt_value(&mut input, &mut output, InputMode::Interactive, "kind")
            .unwrap(),
        Some("12345".to_owned())
    );
    assert_eq!(String::from_utf8(output).unwrap(), "kind> ");
    assert!(session.history().is_empty());

    let mut input = Cursor::new("12345\n");
    let mut output = Vec::new();
    assert!(matches!(
        session.prompt_value(&mut input, &mut output, InputMode::Script, "kind"),
        Err(ShellError::NonInteractivePrompt)
    ));
    assert!(output.is_empty());
    let mut remaining = String::new();
    input.read_to_string(&mut remaining).unwrap();
    assert_eq!(remaining, "12345\n");
    assert!(session.history().is_empty());
}

#[test]
fn shared_command_omissions_prompt_interactively_and_do_not_consume_replay_lines() {
    let mut interactive = session();
    let mut input = Cursor::new("account new\nalice\nquit\n");
    let mut output = Vec::new();
    interactive
        .run(
            &mut input,
            &mut output,
            InputMode::Interactive,
            OutputFormat::Human,
            |_, _, _, _, _| unreachable!(),
        )
        .unwrap();
    assert_eq!(interactive.history(), ["account new", "quit"]);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("account-alias> "));
    assert!(output.contains("account-created"));

    let mut scripted = session();
    let mut input = Cursor::new("account new\nalice\n");
    let mut output = Vec::new();
    assert!(matches!(
        scripted.run(
            &mut input,
            &mut output,
            InputMode::Script,
            OutputFormat::JsonLines,
            |_, _, _, _, _| unreachable!(),
        ),
        Err(ShellError::NonInteractivePrompt)
    ));
    let mut remaining = String::new();
    input.read_to_string(&mut remaining).unwrap();
    assert_eq!(remaining, "alice\n");
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("\"kind\":\"shell-refused\"")
    );
}

#[test]
fn interactive_jsonl_is_refused_before_any_prompt_or_rendering() {
    let mut interactive = session();
    let mut input = Cursor::new("quit\n");
    let mut output = Vec::new();
    assert!(matches!(
        interactive.run(
            &mut input,
            &mut output,
            InputMode::Interactive,
            OutputFormat::JsonLines,
            |_, _, _, _, _| unreachable!(),
        ),
        Err(ShellError::InteractiveJsonLines)
    ));
    assert!(output.is_empty());
}

#[test]
fn input_modes_use_one_dispatcher_without_a_pty() {
    let mut scripted = session();
    let mut script = Cursor::new(
        "account new alice\nrelay add group ws://127.0.0.1:18101\naccount list\nrelay list\ndump\nquit\n",
    );
    let mut output = Vec::new();
    scripted
        .run(
            &mut script,
            &mut output,
            InputMode::Script,
            OutputFormat::JsonLines,
            |_, _, _, _, _| unreachable!(),
        )
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("e2e>"));
    assert!(output.contains("\"kind\":\"account-created\""));
    assert!(output.contains("\"kind\":\"relay-list\""));

    let mut interactive = session();
    let mut input = Cursor::new("quit\n");
    let mut output = Vec::new();
    interactive
        .run(
            &mut input,
            &mut output,
            InputMode::Interactive,
            OutputFormat::Human,
            |_, _, _, _, _| unreachable!(),
        )
        .unwrap();
    assert!(String::from_utf8(output).unwrap().starts_with("e2e> "));

    let mut failed_domain = session();
    let mut input = Cursor::new("group create room group\nquit\n");
    let mut output = Vec::new();
    let error = failed_domain
        .run(
            &mut input,
            &mut output,
            InputMode::Script,
            OutputFormat::JsonLines,
            |_, _, _, _, _| Err(ShellError::Domain("relay refused room".to_owned())),
        )
        .unwrap_err();
    assert_eq!(error, ShellError::Domain("relay refused room".to_owned()));
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("\"status\":\"failed\"")
    );
}
