//! Physics simulation module
//!
//! Kinematic контроллер, движение, коллизии через Rapier.
//! Архитектура: docs/architecture/physics-architecture.md

pub mod movement;

// Re-export основных типов
pub use movement::{
    KinematicController,
    MovementInput,
    KinematicControllerPlugin,
    spawn_kinematic_character,
};
