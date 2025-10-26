//! AI components

pub mod fsm;

// Tests (separate files with _tests suffix)
#[cfg(test)]
mod fsm_tests;

// Re-export all components
pub use fsm::*;
