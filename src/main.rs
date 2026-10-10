use std::io::{stderr, stdin, stdout};
use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use factory::{cli, ctx};

/// Drives unattended feature delivery: spawns coding-agent workers with
/// context packs, runs the gate itself and keeps the record.
#[derive(Parser)]
#[command(name = "factory", version, about, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start or resume the unattended run of a feature
    Run(cli::run::RunArgs),
    /// Block until the run needs the owner or ends
    Wait(cli::wait::WaitArgs),
    /// Answer an open blocker so its lane resumes
    Answer(cli::answer::AnswerArgs),
    /// Show the board of the live run
    Status(cli::status::StatusArgs),
    /// Stop the live run
    Stop(cli::stop::StopArgs),
    /// Print the metrics report of a run
    Report(cli::report::ReportArgs),
    /// Code lookups for workers: definitions, references, outlines
    Ctx {
        #[command(subcommand)]
        verb: ctx::Verb,
    },
    /// The PreToolUse hook the driver injects into every worker
    Hook,
    /// The detached driver `factory run` starts; not for hands
    #[command(hide = true)]
    Daemon(cli::run::DaemonArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(a) => cli::run::run(&a, &mut stdout(), &mut stderr()),
        Command::Stop(a) => cli::stop::stop(&a, &mut stdout()),
        Command::Daemon(a) => cli::run::daemon(&a, &mut stdout(), &mut stderr()),
        Command::Answer(a) => cli::answer::answer(&a, &mut stdout()),
        Command::Report(a) => cli::report::report(&a, &mut stdout()),
        Command::Wait(a) => cli::wait::wait(&a, &mut stdout()),
        Command::Status(a) => cli::status::status(&a, &mut stdout()),
        Command::Ctx { verb } => ctx::run(&verb, Path::new("."), &mut stdout(), &mut stderr()),
        Command::Hook => cli::hook::hook(&mut stdin(), &mut stdout(), &mut stderr()),
    };
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("factory: {e}");
            ExitCode::from(1)
        }
    }
}
