//! Combat systems (strategic layer logic)

pub mod melee;
pub mod stamina;
pub mod weapon;
pub mod damage;

// Tests (separate files with _tests suffix)
#[cfg(test)]
mod stamina_tests;
#[cfg(test)]
mod weapon_tests;
#[cfg(test)]
mod damage_tests;

// Re-export all systems
pub use melee::*;
pub use stamina::*;
pub use weapon::*;
pub use damage::*;
