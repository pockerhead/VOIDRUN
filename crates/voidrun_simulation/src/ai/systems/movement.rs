//! AI movement systems.

use bevy::prelude::*;
use crate::components::{Actor, MovementCommand, Stamina};
use crate::combat::WeaponStats;
use crate::ai::AIState;

/// Система: AI movement from state
///
/// Конвертирует AIState → MovementCommand для Godot.
/// ADR-005: Используем StrategicPosition для AI decisions
pub fn ai_movement_from_state(
    mut ai_query: Query<(&AIState, &mut MovementCommand, &crate::StrategicPosition)>,
    _targets_query: Query<&crate::StrategicPosition>,
) {
    for (state, mut command, _strategic_pos) in ai_query.iter_mut() {
        match state {
            AIState::Dead => {
                // Dead — не двигаемся
                if !matches!(*command, MovementCommand::Idle) {
                    *command = MovementCommand::Idle;
                }
            }

            AIState::Idle => {
                if !matches!(*command, MovementCommand::Idle) {
                    *command = MovementCommand::Idle;
                }
            }

            AIState::Patrol { target_position, .. } => {
                // Двигаемся к сгенерированной patrol точке (генерируется в ai_fsm_transitions)
                if let Some(target) = target_position {
                    // Проверяем что команда изменилась — иначе Changed<MovementCommand> спамит
                    if !matches!(*command, MovementCommand::MoveToPosition { target: t } if t == *target) {
                        *command = MovementCommand::MoveToPosition {
                            target: *target,
                        };
                    }
                } else {
                    // Нет target позиции → Idle (будет сгенерирована при следующем тике)
                    if !matches!(*command, MovementCommand::Idle) {
                        *command = MovementCommand::Idle;
                    }
                }
            }

            AIState::Combat { target } => {
                // Следуем за target (FollowEntity для динамического преследования)
                if !matches!(*command, MovementCommand::FollowEntity { target: t } if t == *target) {
                    crate::log(&format!("🏃 AI movement: Combat → FollowEntity {:?}", target));
                    *command = MovementCommand::FollowEntity {
                        target: *target,
                    };
                }
            }

            AIState::Retreat { from_target, .. } => {
                // Тактическое отступление: пятиться назад, но смотреть на врага
                let Some(target_entity) = from_target else {
                    if !matches!(*command, MovementCommand::Idle) {
                        *command = MovementCommand::Idle;
                    }
                    continue;
                };

                // Используем RetreatFrom для тактического отступления
                if !matches!(*command, MovementCommand::RetreatFrom { target: t } if t == *target_entity) {
                    *command = MovementCommand::RetreatFrom {
                        target: *target_entity,
                    };
                }
            }
        }
    }
}

/// Система: AI attack execution
///
/// Генерирует атаки когда в Combat state и target в радиусе.
/// ADR-005: Используем StrategicPosition для distance checks
pub fn ai_attack_execution(
    mut ai_query: Query<(&AIState, &crate::StrategicPosition, &mut WeaponStats, &Stamina)>,
    targets_query: Query<&crate::StrategicPosition>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (state, strategic_pos, mut weapon, stamina) in ai_query.iter_mut() {
        // Обновляем cooldown
        if weapon.cooldown_timer > 0.0 {
            weapon.cooldown_timer -= delta;
        }

        // Атакуем только в Combat state
        let AIState::Combat { target } = state else {
            continue;
        };

        let Ok(target_strategic_pos) = targets_query.get(*target) else {
            continue;
        };

        let current_world_pos = strategic_pos.to_world_position(0.5);
        let target_world_pos = target_strategic_pos.to_world_position(0.5);
        let distance = current_world_pos.distance(target_world_pos);

        // Проверяем: в радиусе, cooldown готов, есть stamina
        const ATTACK_COST: f32 = 20.0;
        if distance <= weapon.attack_radius
            && weapon.cooldown_timer <= 0.0
            && stamina.current >= ATTACK_COST
        {
            // Атака происходит через старую систему (combat systems обрабатывают)
            // Просто сбрасываем cooldown
            weapon.cooldown_timer = weapon.attack_cooldown;

            crate::log(&format!("AI: attacking target {:?}", target));
        }
    }
}

/// Система: collision resolution (отталкивание NPC друг от друга)
///
/// Предотвращает стэкинг actors на одной точке.
/// ADR-005: Используем StrategicPosition, Godot обновит визуалы через PostSpawn
pub fn simple_collision_resolution(
    mut actors: Query<(&mut crate::StrategicPosition, Entity), With<Actor>>,
) {
    let positions: Vec<(Entity, Vec3)> = actors
        .iter()
        .map(|(sp, e)| (e, sp.to_world_position(0.5)))
        .collect();

    for (mut strategic_pos, entity) in actors.iter_mut() {
        let mut push = Vec3::ZERO;
        let current_pos = strategic_pos.to_world_position(0.5);

        for &(other_entity, other_pos) in &positions {
            if other_entity == entity {
                continue;
            }

            let diff = current_pos - other_pos;
            let distance = diff.length();

            // Минимальная дистанция между actors
            const MIN_DISTANCE: f32 = 1.0;

            if distance < MIN_DISTANCE && distance > 0.001 {
                let push_force = (MIN_DISTANCE - distance) / MIN_DISTANCE;
                push += diff.normalize() * push_force * 0.1;
            }
        }

        // Применяем push к StrategicPosition
        if push.length() > 0.001 {
            let new_pos = current_pos + push;
            *strategic_pos = crate::StrategicPosition::from_world_position(new_pos);
        }
    }
}
