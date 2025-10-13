//! AI decision-making module
//!
//! Simple FSM для aggro AI (Фаза 1), Utility AI для faction AI (Фаза 4).
//! Архитектура: docs/arch_backlog.md (#5)

use bevy::prelude::*;

pub mod simple_fsm;
pub mod events;

// Re-export основных типов
pub use simple_fsm::{AIState, AIConfig, SpottedEnemies, ai_react_to_gunfire};
pub use events::{GodotAIEvent, GodotTransformEvent};

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
        app.add_event::<GodotTransformEvent>();

        app.add_systems(
            FixedUpdate,
            (
                sync_strategic_position_from_godot_events, // 0. Event-driven sync (Godot → ECS)
                simple_fsm::handle_actor_death,     // 1. Обработка смерти → Dead state
                simple_fsm::update_spotted_enemies, // 2. Обновляем SpottedEnemies из GodotAIEvent
                simple_fsm::ai_react_to_gunfire,    // 3. AI реакция на звук выстрела (WeaponFired → ActorSpotted)
                simple_fsm::ai_fsm_transitions,     // 4. FSM transitions на основе SpottedEnemies
                simple_fsm::ai_movement_from_state, // 5. Конвертация state → MovementCommand
                simple_fsm::ai_attack_execution,    // 6. Генерация атак
                simple_fsm::simple_collision_resolution, // 7. Отталкивание NPC
            )
                .chain(), // Последовательное выполнение для детерминизма
        );
    }
}

/// Godot Transform → ECS StrategicPosition sync (event-driven)
///
/// ADR-005: Event-driven sync вместо periodic polling.
/// Обрабатывает PostSpawn (после spawn) и PositionChanged (после движения).
pub fn sync_strategic_position_from_godot_events(
    mut actors: Query<&mut crate::StrategicPosition>,
    mut transform_events: EventReader<GodotTransformEvent>,
) {
    for event in transform_events.read() {
        let (entity, position) = match event {
            GodotTransformEvent::PostSpawn { entity, position } => {
                crate::log(&format!("PostSpawn: entity {:?} at {:?}", entity, position));
                (*entity, *position)
            }
            GodotTransformEvent::PositionChanged { entity, position } => (*entity, *position),
        };

        let Ok(mut strategic_pos) = actors.get_mut(entity) else {
            continue;
        };

        // Пересчитываем StrategicPosition из точной Godot позиции
        let corrected = crate::StrategicPosition::from_world_position(position);

        // Обновляем только если изменилось (избегаем Changed<StrategicPosition> спама)
        if strategic_pos.chunk != corrected.chunk || strategic_pos.local_offset != corrected.local_offset {
            *strategic_pos = corrected;
        }
    }
}
