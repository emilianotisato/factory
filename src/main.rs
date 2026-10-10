use std::io::{stderr, stdout};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use factory::cli;

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
    Wait(Passthrough),
    /// Answer an open blocker so its lane resumes
    Answer(cli::answer::AnswerArgs),
    /// Show the board of the live run
    Status(Passthrough),
    /// Stop the live run
    Stop(cli::stop::StopArgs),
    /// Print the metrics report of a run
    Report(cli::report::ReportArgs),
    /// Code lookups for workers: definitions, references, outlines
    Ctx {
        #[command(subcommand)]
        verb: CtxVerb,
    },
    /// The PreToolUse hook the driver injects into every worker
    Hook(Passthrough),
    /// The detached driver `factory run` starts; not for hands
    #[command(hide = true)]
    Daemon(cli::run::DaemonArgs),
}

#[derive(Subcommand)]
enum CtxVerb {
    /// Where a symbol is defined
    Def(Passthrough),
    /// Where a symbol is used
    Refs(Passthrough),
    /// The definitions a file holds
    Outline(Passthrough),
}

/// Arguments are accepted and ignored until the slice that builds the
/// subcommand gives them a shape.
#[derive(Args)]
struct Passthrough {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    args: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(a) => cli::run::run(&a, &mut stdout(), &mut stderr()),
        Command::Stop(a) => cli::stop::stop(&a, &mut stdout()),
        Command::Daemon(a) => cli::run::daemon(&a, &mut stdout(), &mut stderr()),
        Command::Answer(a) => cli::answer::answer(&a, &mut stdout()),
        Command::Report(a) => cli::report::report(&a, &mut stdout()),
        Command::Wait(p) | Command::Status(p) => return not_built(10, &p.args),
        Command::Ctx { verb } => match verb {
            CtxVerb::Def(p) | CtxVerb::Refs(p) => return not_built(11, &p.args),
            CtxVerb::Outline(p) => return not_built(12, &p.args),
        },
        Command::Hook(p) => return not_built(13, &p.args),
    };
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("factory: {e}");
            ExitCode::from(1)
        }
    }
}

fn not_built(slice: u8, args: &[String]) -> ExitCode {
    eprintln!(
        "factory: not built yet (slice {slice:02}) — {} argument(s) ignored",
        args.len()
    );
    ExitCode::from(2)
}
