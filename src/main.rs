use clap::{Args, Parser, Subcommand};
use std::process::ExitCode;

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
    Run(Passthrough),
    /// Block until the run needs the owner or ends
    Wait(Passthrough),
    /// Answer an open blocker so its lane resumes
    Answer(Passthrough),
    /// Show the board of the live run
    Status(Passthrough),
    /// Stop the live run
    Stop(Passthrough),
    /// Print the metrics report of a run
    Report(Passthrough),
    /// Code lookups for workers: definitions, references, outlines
    Ctx {
        #[command(subcommand)]
        verb: CtxVerb,
    },
    /// The PreToolUse hook the driver injects into every worker
    Hook(Passthrough),
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
    let (slice, args) = match cli.command {
        Command::Run(p) | Command::Stop(p) => (3, p.args),
        Command::Answer(p) => (4, p.args),
        Command::Report(p) => (8, p.args),
        Command::Wait(p) | Command::Status(p) => (10, p.args),
        Command::Ctx { verb } => match verb {
            CtxVerb::Def(p) | CtxVerb::Refs(p) => (11, p.args),
            CtxVerb::Outline(p) => (12, p.args),
        },
        Command::Hook(p) => (13, p.args),
    };
    not_built(slice, &args)
}

fn not_built(slice: u8, args: &[String]) -> ExitCode {
    eprintln!(
        "factory: not built yet (slice {slice:02}) — {} argument(s) ignored",
        args.len()
    );
    ExitCode::from(2)
}
