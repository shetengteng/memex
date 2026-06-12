//! Read-only / analytics queries that don't fit into the per-table
//! CRUD modules under [`super::db`]. Subdivided by topic:
//!
//! - [`doctor`]: health / schema / source / FTS checks (CLI `memex
//!   doctor`, Settings → System tab)
//! - [`stats`]: timeline + breakdown + project summaries (Dashboard
//!   overview)
//! - [`workload`]: daily / heatmap / by-adapter / by-project rollups
//!   (Dashboard → Workload tab)
//! - [`dto`]: shared `#[derive(Serialize)]` types that cross the
//!   IPC boundary
//!
//! Each submodule extends `impl Db` with the methods it owns.

mod doctor;
mod dto;
mod stats;
#[cfg(test)]
mod tests;
mod workload;

pub use dto::*;
