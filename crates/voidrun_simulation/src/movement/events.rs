//! Movement events

use bevy::prelude::*;

/// Event: намерение прыгнуть (jump intent)
///
/// Генерируется:
/// - Player input system (Space key)
/// - AI system (для NPC, если нужно)
///
/// Обрабатывается:
/// - apply_safe_velocity_system (Godot layer): проверяет is_on_floor() и применяет jump velocity
#[derive(Event, Debug, Clone)]
pub struct JumpIntent {
    pub entity: Entity,
}
