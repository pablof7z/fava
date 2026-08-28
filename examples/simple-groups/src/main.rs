//! Interactive and replayable real-relay simple-group E2E application.

mod app;
mod group_events;
mod group_metadata;
mod groups;
mod reads;
mod saved_lists;
mod support;

use std::fs::File;
use std::io::{BufReader, IsTerminal as _, stdin, stdout};
use std::path::PathBuf;

use app::App;
use e2e_support::{E2eSession, InputMode, Limits, OutputFormat};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let options = Options::parse()?;
    let fava = support::assemble()?;
    let mut session = E2eSession::new(Limits::standard(), fava.clone());
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
        let mode = if stdin().is_terminal() {
            InputMode::Interactive
        } else {
            InputMode::Script
        };
        let mut input = BufReader::new(stdin().lock());
        run(
            &mut session,
            &mut app,
            &mut input,
            &mut output,
            mode,
            options.format,
        )?;
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
        |session, words, input, output, mode| app.execute(session, words, input, output, mode),
    )?;
    Ok(())
}

struct Options {
    script: Option<PathBuf>,
    format: OutputFormat,
}

impl Options {
    fn parse() -> AppResult<Self> {
        let mut script = None;
        let mut format = OutputFormat::Human;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--script" => {
                    script = Some(PathBuf::from(
                        arguments.next().ok_or("--script requires a command file")?,
                    ));
                }
                "--jsonl" => format = OutputFormat::JsonLines,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown option {argument:?}").into()),
            }
        }
        Ok(Self { script, format })
    }
}

fn print_help() {
    println!("simple-groups [--jsonl] [--script <path>]");
    println!("See README.md for the shared and simple-group command grammar.");
}
