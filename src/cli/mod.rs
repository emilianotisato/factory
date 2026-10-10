//! The subcommands, one module each. Each takes its parsed `clap` arguments
//! and the writer for stdout, and returns the exit code, so tests drive them
//! in-process and `main` stays a dispatch table.
pub mod answer;
pub mod run;
pub mod stop;
