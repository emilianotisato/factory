//! `factory` drives unattended feature delivery: it spawns coding-agent
//! workers with context packs, runs the gate itself and keeps the record.
//!
//! Slice 01 of PRD-001 ships the CLI surface and the test harness under
//! `tests/`; the driver itself lands slice by slice (`docs/features/001-driver`).
