//! Interactive and replayable real-relay simple-group E2E application.

mod app;
mod group_events;
mod group_metadata;
mod groups;
mod reads;
mod saved_lists;
mod support;
mod terminal;
mod terminal_completion;
mod terminal_editor;
mod terminal_history;

use std::fs::File;
use std::io::{BufReader, IsTerminal as _, stdin, stdout};
use std::path::PathBuf;
use std::time::Instant;

use app::App;
use e2e_support::{CommandResult, E2eSession, InputMode, Limits, OutputFormat, ShellError};
use reedline::Signal;
use terminal::Terminal;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let options = Options::parse()?;
    let fava = support::assemble()?;
    let limits = Limits::new(4, 8, 8, 16, 512, 4_096, 16, 24, 32)?;
    let mut session = E2eSession::new(limits, fava.clone());
    let mut app = App::new(fava);
    let mut output = stdout().lock();

    if let Some(path) = options.script {
        let mut input = BufReader::new(File::open(path)?);
        run(
            &mut session,
            &mut app,
            &mut input,
            &mut output,
            InputMode::Script,
            options.format,
        )?;
    } else {
        let stdin_is_terminal = stdin().is_terminal();
        let interactive = stdin_is_terminal
            && stdout().is_terminal()
            && matches!(options.format, OutputFormat::Human);
        if interactive {
            run_interactive(&mut session, &mut app, &mut output, options.color_enabled())?;
        } else {
            let mode = if stdin_is_terminal {
                InputMode::Interactive
            } else {
                InputMode::Script
            };
            // Do not hold the global stdin lock across the REPL: protected
            // account import acquires it briefly for its no-echo terminal read.
            let mut input = BufReader::new(stdin());
            run(
                &mut session,
                &mut app,
                &mut input,
                &mut output,
                mode,
                options.format,
            )?;
        }
    }
    Ok(())
}

fn run<R: std::io::BufRead, W: std::io::Write>(
    session: &mut E2eSession,
    app: &mut App,
    input: &mut R,
    output: &mut W,
    mode: InputMode,
    format: OutputFormat,
) -> AppResult<()> {
    session.run(
        input,
        output,
        mode,
        format,
        |session, words, input, output, mode| {
            let mut prompt = |label: &str| session.prompt_value(input, output, mode, label);
            app.execute(session, words, &mut prompt)
        },
    )?;
    Ok(())
}

fn run_interactive(
    session: &mut E2eSession,
    app: &mut App,
    output: &mut impl std::io::Write,
    color: bool,
) -> AppResult<()> {
    let mut terminal = Terminal::new(Limits::standard(), color)?;
    terminal.write_intro(output)?;
    loop {
        let signal = terminal.read_command(
            session.selected_account_alias(),
            app.selected_group.as_deref(),
            session.relay_count(),
        )?;
        let Signal::Success(line) = signal else {
            terminal.render_cancelled(output)?;
            if matches!(signal, Signal::CtrlD) {
                return Ok(());
            }
            continue;
        };
        let started = Instant::now();
        let mut prompt = |label: &str| terminal.read_value(label);
        let execution = session.execute_line_with_domain_prompt(
            &line,
            InputMode::Interactive,
            &mut prompt,
            |session, words, prompt| app.execute(session, words, prompt),
        );
        let (result, quit) = interactive_result(execution);
        terminal.render_result(output, &result, started.elapsed())?;
        if quit {
            return Ok(());
        }
    }
}

fn interactive_result(result: Result<CommandResult, ShellError>) -> (CommandResult, bool) {
    match result {
        Ok(result) => {
            let quit = result.kind() == "quit";
            (result, quit)
        }
        Err(ShellError::CommandFailed { result }) => (result, false),
        Err(error @ ShellError::Domain(_)) => (
            CommandResult::failed("domain-failed", bounded_summary(&error.to_string())),
            false,
        ),
        Err(error) => (
            CommandResult::refused("shell-refused", bounded_summary(&error.to_string())),
            false,
        ),
    }
}

fn bounded_summary(value: &str) -> String {
    value
        .chars()
        .scan(0usize, |bytes, character| {
            let next = bytes.saturating_add(character.len_utf8());
            (next <= 4_096).then(|| {
                *bytes = next;
                character
            })
        })
        .collect()
}

struct Options {
    script: Option<PathBuf>,
    format: OutputFormat,
    no_color: bool,
}

impl Options {
    fn parse() -> AppResult<Self> {
        let mut script = None;
        let mut format = OutputFormat::Human;
        let mut no_color = false;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--script" => {
                    script = Some(PathBuf::from(
                        arguments.next().ok_or("--script requires a command file")?,
                    ));
                }
                "--jsonl" => format = OutputFormat::JsonLines,
                "--no-color" => no_color = true,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown option {argument:?}").into()),
            }
        }
        Ok(Self {
            script,
            format,
            no_color,
        })
    }

    fn color_enabled(&self) -> bool {
        !self.no_color && std::env::var_os("NO_COLOR").is_none()
    }
}

fn print_help() {
    println!("simple-groups [--jsonl] [--no-color] [--script <path>]");
    println!(
        "Interactive: Tab completion, usage hints, syntax highlighting, and in-process history."
    );
    println!(
        "Commands: account, relay, group, saved-list, status, routes, receipt, diagnostics, capture, dump, quit."
    );
    println!(
        "Scripts/non-TTY use the identical grammar and plain deterministic human or JSONL output."
    );
    println!("See README.md for the shared and simple-group command grammar.");
}
