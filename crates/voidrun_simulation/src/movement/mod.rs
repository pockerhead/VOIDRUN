//! Movement domain — навигация и команды перемещения
//!
//! Содержит:
//! - MovementCommand (high-level intent для Godot NavigationAgent)
//! - NavigationState (состояние навигации)
//! - MovementSpeed (скорость движения)
//! - JumpIntent (event для прыжка)

pub mod components;
pub mod events;

// Re-export all components and events
pub use components::*;
pub use events::*;
