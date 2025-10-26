//! Shooting domain — прицеливание и стрельба (player)
//!
//! Содержит:
//! - AimMode (Hip Fire / ADS состояния + transitions)
//! - ToggleADSIntent (event для переключения режима прицеливания)
//! - ease_out_cubic (easing function)

pub mod components;

// Re-export all components and functions
pub use components::*;
