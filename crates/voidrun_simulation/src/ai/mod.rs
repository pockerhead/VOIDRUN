//! AI decision-making module (domain-driven architecture)
//!
//! Simple FSM для aggro AI (Фаза 1), Utility AI для faction AI (Фаза 4).
//! Архитектура: docs/arch_backlog.md (#5)

use bevy::prelude::*;

// Domain modules
pub mod components;
pub mod systems;
pub mod events;

// Re-export components
pub use components::{AIState, AIConfig, SpottedEnemies};

// Re-export systems
pub use systems::{
    // FSM systems
    update_spotted_enemies, ai_fsm_transitions,
    // Movement systems
    ai_movement_from_state, ai_attack_execution, simple_collision_resolution,
    // Reaction systems
    handle_actor_death, react_to_damage, ai_react_to_gunfire,
};

// Re-export events
pub use events::{GodotAIEvent, GodotTransformEvent, GodotNavigationEvent, CombatAIEvent};

/// AI Plugin
///
/// Регистрирует AI системы в FixedUpdate для детерминизма.
/// Порядок выполнения:
/// 1. ai_fsm_transitions — обновление FSM state
/// 2. ai_movement_from_state — конвертация state → MovementCommand
/// 3. simple_collision_resolution — отталкивание NPC друг от друга
///
/// NOTE: Атаки генерируются через combat systems (ai_melee_attack_intent, ai_weapon_fire_intent)
pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        // Регистрируем AI events (Godot → ECS, ECS → ECS)
        app.add_event::<GodotAIEvent>();
        app.add_event::<GodotTransformEvent>();
        app.add_event::<GodotNavigationEvent>();
        app.add_event::<CombatAIEvent>();
        app.add_systems(
            FixedUpdate,
            (
                sync_strategic_position_from_godot_events, // 0. Event-driven sync (Godot → ECS)
                handle_actor_death,          // 1. Обработка смерти → Dead state
                update_spotted_enemies,      // 2. Обновляем SpottedEnemies из GodotAIEvent
                react_to_damage,             // 3. AI реакция на урон (DamageDealt → FollowEntity)
                ai_react_to_gunfire,         // 4. AI реакция на звук выстрела (WeaponFired → ActorSpotted)
                ai_fsm_transitions,          // 5. FSM transitions на основе SpottedEnemies
                ai_movement_from_state,      // 6. Конвертация state → MovementCommand
                // УДАЛЕНО: ai_attack_execution (заменён на ai_melee_attack_intent в combat systems)
                simple_collision_resolution, // 7. Отталкивание NPC
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
        let (entity, position ) = match event {
            GodotTransformEvent::PostSpawn { entity, position } => {
                crate::log(&format!("PostSpawn: entity {:?} at {:?}", entity, position));
                (*entity, Some(*position))
            }
            GodotTransformEvent::PositionChanged { entity, position } => (*entity, Some(*position))
        };

        let Ok(mut strategic_pos) = actors.get_mut(entity) else {
            continue;
        };

        let Some(position) = position else {
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

pub fn handle_navigation_failed(
    mut actors: Query<(Entity, &mut AIState)>,
    mut navigation_events: EventReader<GodotNavigationEvent>,
) {
    for event in navigation_events.read() {
        let GodotNavigationEvent::NavigationFailed { entity } = event else {
            continue;
        };
        let Ok((entity, mut state)) = actors.get_mut(*entity) else {
            continue;
        };
        *state = AIState::Idle;
    }
}