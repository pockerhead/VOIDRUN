//! Player input events
//!
//! События генерируются из Godot Input API (PlayerInputController)
//! и обрабатываются ECS systems.

use bevy::prelude::{Event, Vec2};

/// Player input event - генерируется каждый frame когда есть player input
///
/// # Архитектура
/// - Emit: PlayerInputController (Godot node) в `process()`
/// - Consume: player_movement_system, player_combat_input (ECS systems)
///
/// # Fields
/// - `move_direction`: WASD input (normalized, Vec2::ZERO если нет движения)
/// - `sprint`: Shift key (unlimited sprint, stamina не тратится пока)
/// - `jump`: Space key (just_pressed)
/// - `attack`: LMB (just_pressed)
/// - `parry`: RMB (just_pressed)
///
/// # Примечание
/// Mouse look пока НЕ включён (камера будет позже)
#[derive(Event, Debug, Clone, Copy, Default)]
pub struct PlayerInputEvent {
    /// WASD movement direction (normalized)
    /// - (0, 1) = forward
    /// - (0, -1) = backward
    /// - (-1, 0) = left
    /// - (1, 0) = right
    pub move_direction: Vec2,

    /// Sprint key (Shift) - пока unlimited (stamina не тратится)
    pub sprint: bool,

    /// Jump key (Space) - just_pressed
    pub jump: bool,

    /// Attack key (LMB) - just_pressed
    pub attack: bool,

    /// Parry key (RMB) - just_pressed
    pub parry: bool,
}
