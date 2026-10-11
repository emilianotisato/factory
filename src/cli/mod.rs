//! The subcommands, one module each. Each takes its parsed `clap` arguments
//! and the writer for stdout, and returns the exit code, so tests drive them
//! in-process and `main` stays a dispatch table.
pub mod answer;
pub mod hook;
pub mod report;
pub mod run;
pub mod status;
pub mod stop;
pub mod wait;
