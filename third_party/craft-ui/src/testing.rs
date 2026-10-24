/// The result every test returns, so the crate's strict lints
/// (`#![deny(clippy::unwrap_used, …)]`) can be honored with `?`
/// instead of `unwrap()` (DiagramCraft #61).
pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;
