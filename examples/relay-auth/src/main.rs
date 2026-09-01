//! Interactive and replayable NIP-42 relay-authentication E2E application.

mod app;
mod render;
mod support;
mod terminal;

use std::fs::File;
use std::io::{BufReader, IsTerminal as _, stdin, stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use app::App;
use e2e_support::{CommandResult, E2eSession, InputMode, Limits, OutputFormat, ShellError};
use fava_auth::AuthenticationDecision;
use reedline::Signal;
use support::SwitchablePolicy;
use terminal::Terminal;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let options = Options::parse()?;
    // Nothing is authenticated until a person or a scenario line asks for it:
    // the engine starts under `Decline`, and `policy set` changes the answer
    // every later challenge on this process gets.
    let policy = Arc::new(SwitchablePolicy::new(AuthenticationDecision::Decline));
    let fava = support::assemble(Arc::clone(&policy))?;
    let mut session = E2eSession::new(
        Limits::new(8, 8, 16, 32, 1_024, 4_096, 24, 32, 32)?,
        fava.clone(),
    );
    let mut app = App::new(fava, policy);
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
        let interactive = stdin().is_terminal()
            && stdout().is_terminal()
            && matches!(options.format, OutputFormat::Human);
        if interactive {
            run_interactive(&mut session, &mut app, &mut output, options.color_enabled())?;
        } else {
            let mode = if stdin().is_terminal() {
                InputMode::Interactive
            } else {
                InputMode::Script
            };
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
            session.relay_count(),
            app.query_count(),
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
            CommandResult::failed("domain-failed", bounded(&error.to_string())),
            false,
        ),
        Err(error) => (
            CommandResult::refused("shell-refused", bounded(&error.to_string())),
            false,
        ),
    }
}

fn bounded(value: &str) -> String {
    value.chars().take(4_096).collect()
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
    println!("relay-auth [--script FILE] [--jsonl] [--no-color]");
    println!("run interactively or replay ordinary REPL command lines");
}
