//! `factory` drives unattended feature delivery: it spawns coding-agent
//! workers with context packs, runs the gate itself and keeps the record.
//!
//! Slice 02 of PRD-001 lays the contract every later slice builds on: the
//! board is read through [`docs`], a run lives in [`run`] (its directory,
//! `state.json`, `events.jsonl`), every event goes through [`events::emit`]
//! and lands in the SQLite store in [`db`], and [`config`] reads
//! `config.toml`. The driver itself lands slice by slice
//! (`docs/features/001-driver`).
pub mod answers;
pub mod checkpoint;
pub mod ci;
pub mod cli;
pub mod clock;
pub mod closeout;
pub mod cockpit;
pub mod commit;
pub mod config;
pub mod ctx;
pub mod daemon;
pub mod db;
pub mod docs;
pub mod error;
pub mod events;
pub mod gatecache;
pub mod lanes;
pub mod measure;
pub mod metrics;
pub mod pack;
pub mod paths;
pub mod quota;
pub mod run;
pub mod scheduler;
pub mod shell;
pub mod stream;
pub mod worker;

pub use error::{Error, Result};
