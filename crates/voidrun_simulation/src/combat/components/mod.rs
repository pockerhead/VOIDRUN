//! Combat components

pub mod melee;
pub mod weapon;
pub mod stamina;

// Tests (separate files with _tests suffix)
#[cfg(test)]
mod weapon_tests;

// Re-export all components
pub use melee::*;
pub use weapon::*;
pub use stamina::*;
