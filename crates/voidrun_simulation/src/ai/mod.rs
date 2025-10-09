//! AI decision-making module
//!
//! Simple FSM для aggro AI (Фаза 1), Utility AI для faction AI (Фаза 4).
//! Архитектура: docs/arch_backlog.md (#5)

use bevy::prelude::*;

pub mod simple_fsm;

// Re-export основных типов
pub use simple_fsm::{AIState, AIConfig};

/// AI Plugin
///
/// Регистрирует AI системы в FixedUpdate для детерминизма.
/// Порядок выполнения:
/// 1. ai_fsm_transitions — обновление FSM state
/// 2. ai_movement_from_state — конвертация state → MovementInput
/// 3. ai_attack_execution — генерация AttackStarted событий
/// 4. simple_collision_resolution — отталкивание NPC друг от друга
pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                simple_fsm::ai_fsm_transitions,
                simple_fsm::ai_movement_from_state,
                simple_fsm::ai_attack_execution,
                simple_fsm::simple_collision_resolution,
            )
                .chain(), // Последовательное выполнение для детерминизма
        );
    }
}
