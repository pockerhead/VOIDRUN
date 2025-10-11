//! AI decision-making module
//!
//! Simple FSM для aggro AI (Фаза 1), Utility AI для faction AI (Фаза 4).
//! Архитектура: docs/arch_backlog.md (#5)

use bevy::prelude::*;

pub mod simple_fsm;
pub mod events;

// Re-export основных типов
pub use simple_fsm::{AIState, AIConfig, SpottedEnemies};
pub use events::GodotAIEvent;

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
        // Регистрируем AI events
        app.add_event::<GodotAIEvent>();

        app.add_systems(
            FixedUpdate,
            (
                simple_fsm::handle_actor_death,     // 0. Обработка смерти → Dead state
                simple_fsm::update_spotted_enemies, // 1. Обновляем SpottedEnemies из GodotAIEvent
                simple_fsm::ai_fsm_transitions,     // 2. FSM transitions на основе SpottedEnemies
                simple_fsm::ai_movement_from_state, // 3. Конвертация state → MovementCommand
                simple_fsm::ai_attack_execution,    // 4. Генерация атак
                simple_fsm::simple_collision_resolution, // 5. Отталкивание NPC
            )
                .chain(), // Последовательное выполнение для детерминизма
        );
    }
}
