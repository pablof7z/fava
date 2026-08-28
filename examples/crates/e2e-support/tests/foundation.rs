//! Private E2E support behavioral coverage.

use std::io::{Cursor, IsTerminal as _, Read as _};

use e2e_support::{
    Account, CommandResult, E2eSession, InputMode, Limits, OutputFormat, ShellError,
    parse_public_key,
};

const ALICE: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn session() -> E2eSession {
    E2eSession::new(
        Limits::standard(),
        [Account::new("alice", parse_public_key(ALICE).unwrap())],
    )
    .unwrap()
}

#[test]
fn interactive_and_scripted_execution_share_aliases_selection_capture_and_jsonl() {
    let mut session = session();
    let result = session
        .execute_line(
            "relay add group ws://127.0.0.1:18101",
            |_, _| unreachable!(),
        )
        .unwrap();
    assert_eq!(result.kind(), "relay-added");

    session
        .execute_line("account use alice", |_, _| unreachable!())
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
    assert_eq!(created.field("group"), Some("room"));

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
    assert!(dump.field("captures").unwrap().contains("group-name"));

    let json = OutputFormat::JsonLines.render(&created).unwrap();
    assert!(json.contains("\"kind\":\"group-created\""));
    assert!(json.ends_with('\n'));
}

#[test]
fn secret_input_never_enters_history_or_rendered_script_output() {
    let mut session = session();
    if !std::io::stdin().is_terminal() {
        assert!(matches!(
            session.prompt_secret("private key: "),
            Err(ShellError::NonInteractiveSecretPrompt)
        ));
    }
    let secret = "nsec1never-retain-this";
    let mut script = Cursor::new(format!(
        "account use alice\ngroup event publish --kind 12345 {secret}\n"
    ));
    let mut output = Vec::new();
    assert!(matches!(
        session.run(
            &mut script,
            &mut output,
            InputMode::Script,
            OutputFormat::JsonLines,
            |_, _, _, _, _| unreachable!(),
        ),
        Err(ShellError::SecretOnCommandLine)
    ));
    assert_eq!(session.history(), ["account use alice"]);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("\"kind\":\"shell-refused\""));
    assert!(!output.contains(secret));
    assert!(matches!(
        CommandResult::success("safe", "safe").with_field("token", "value"),
        Err(ShellError::SensitiveResultField { .. })
    ));
}

#[test]
fn bounds_refuse_before_retaining_external_input() {
    let mut session = E2eSession::new(
        Limits::new(1, 1, 1, 8, 256, 64, 3, 8, 32).unwrap(),
        [Account::new("alice", parse_public_key(ALICE).unwrap())],
    )
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
        [Account::new("alice", parse_public_key(ALICE).unwrap())],
    )
    .unwrap();
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
fn input_modes_use_one_dispatcher_without_a_pty() {
    let mut scripted = session();
    let mut script = Cursor::new(
        "relay add group ws://127.0.0.1:18101\naccount use alice\ngroup list\ndump\nquit\n",
    );
    let mut output = Vec::new();
    scripted
        .run(
            &mut script,
            &mut output,
            InputMode::Script,
            OutputFormat::JsonLines,
            |_, words, _, _, _| {
                assert_eq!(words, ["group", "list"]);
                CommandResult::success("group-list", "domain command").with_field("groups", "")
            },
        )
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("e2e>"));
    assert!(output.contains("\"kind\":\"group-list\""));

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
