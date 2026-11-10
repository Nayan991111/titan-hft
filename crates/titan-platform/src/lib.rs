//! Platform layer for latency measurement on Apple silicon (macOS, arm64).
//!
//! Everything here is usable on Linux as well so the workspace builds and the
//! portable parts can be tested anywhere; macOS-only facilities report
//! themselves as unavailable instead of failing to compile.

pub mod alloc_count;
pub mod clock;
pub mod kperf;
pub mod mem;
pub mod qos;
pub mod rusage;
