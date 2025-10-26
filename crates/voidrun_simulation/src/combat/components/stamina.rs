//! Stamina-related components and constants.

use bevy::prelude::*;

/// Стоимость различных действий (stamina points)
pub const ATTACK_COST: f32 = 30.0;
pub const BLOCK_COST: f32 = 20.0;
pub const DODGE_COST: f32 = 25.0; // Для будущего

/// Exhaustion состояние (опционально)
///
/// Когда stamina падает ниже порога, entity получает debuff:
/// - Медленнее движение
/// - Меньше урона
/// - Дольше regen
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Exhausted {
    /// Movement speed multiplier (0.5 = half speed)
    pub movement_penalty: f32,
}

impl Default for Exhausted {
    fn default() -> Self {
        Self {
            movement_penalty: 0.7, // 30% slower
        }
    }
}
