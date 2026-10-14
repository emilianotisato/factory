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
    /// Write a run's numbers into its feature's retro.md
    Retro(cli::retro::RetroArgs),
    /// Find plan ids and history in added code and comments
    Leaks(cli::leaks::LeaksArgs),
    /// Ask a reviewer from another vendor
    Review {
        #[command(subcommand)]
        verb: cli::review::ReviewVerb,
    },
    /// Code lookups for workers: definitions, references, outlines
    Ctx {
        #[command(subcommand)]
        verb: ctx::Verb,
    },
    /// The PreToolUse hook the driver injects into every worker
    Hook,
    /// Set a repo up: the docs layout, the skill family, the convention block
    Init,
    /// Bring this repo's skills to the binary's bundle
    Skills {
        #[command(subcommand)]
        verb: cli::skills::SkillsVerb,
    },
    /// Create, seed and set up this repo's lanes
    Lanes {
        #[command(subcommand)]
        verb: cli::lanes::LanesVerb,
    },
    /// Replace this binary with the latest release
    Upgrade(cli::upgrade::UpgradeArgs),
    /// The feature-docs board: native on v3, `docs.mjs` on a v2 repo
    #[command(disable_help_flag = true)]
    Docs {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
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
        Command::Retro(a) => cli::retro::retro(&a, &mut stdout()),
        Command::Leaks(a) => cli::leaks::leaks(&a, &mut stdout(), &mut stderr()),
        Command::Wait(a) => cli::wait::wait(&a, &mut stdout(), &mut stderr()),
        Command::Status(a) => cli::status::status(&a, &mut stdout(), &mut stderr()),
        Command::Review { verb } => cli::review::review(&verb, &mut stdout(), &mut stderr()),
        Command::Ctx { verb } => ctx::run(&verb, Path::new("."), &mut stdout(), &mut stderr()),
        Command::Hook => cli::hook::hook(&mut stdin(), &mut stdout(), &mut stderr()),
        Command::Init => cli::docs::docs(&["init".to_string()], &mut stdout(), &mut stderr()),
        Command::Skills { verb } => cli::skills::skills(&verb, &mut stdout()),
        Command::Lanes { verb } => cli::lanes::lanes(&verb, &mut stdout()),
        Command::Upgrade(a) => cli::upgrade::upgrade(&a, &mut stdout()),
        Command::Docs { args } => cli::docs::docs(&args, &mut stdout(), &mut stderr()),
    };
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("factory: {e}");
            ExitCode::from(1)
        }
    }
}
