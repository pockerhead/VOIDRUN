//! FSM AI systems (state transitions, spotted enemies tracking).

use bevy::prelude::*;
use crate::components::{Actor, Health, Stamina};
use crate::ai::{GodotAIEvent, AIState, SpottedEnemies, AIConfig};

/// Система: обновление SpottedEnemies из GodotAIEvent
///
/// Читает ActorSpotted/ActorLost events → обновляет SpottedEnemies компонент.
/// Также очищает мёртвые entities из списка (VisionCone не отправляет ActorLost при смерти).
/// Фильтрация по фракциям: добавляем только врагов (разные faction_id).
pub fn update_spotted_enemies(
    mut ai_query: Query<(&mut SpottedEnemies, &Actor)>,
    mut ai_events: EventReader<GodotAIEvent>,
    actors: Query<&Actor>, // Для получения Actor по Entity
    potential_targets: Query<&Health>, // Для проверки что target жив
) {
    for event in ai_events.read() {
        match event {
            GodotAIEvent::EnemyWindupVisible { .. } => {
                // Skip: handled by ai_melee_combat_decision system
                continue;
            }
            GodotAIEvent::ActorSpotted { observer, target } => {
                // Получаем observer actor
                let Ok((mut spotted, observer_actor)) = ai_query.get_mut(*observer) else {
                    continue;
                };

                // Получаем target actor через Query::get (O(1) lookup)
                let Ok(target_actor) = actors.get(*target) else {
                    continue;
                };

                // Проверяем фракции: добавляем только врагов
                if observer_actor.faction_id == target_actor.faction_id {
                    // Союзник — игнорируем
                    continue;
                }

                // Враг — добавляем в список
                if !spotted.enemies.contains(target) {
                    spotted.enemies.push(*target);
                    crate::log(&format!(
                        "👁️ ActorSpotted: {:?} spotted enemy {:?} (faction {} vs {})",
                        observer, target, observer_actor.faction_id, target_actor.faction_id
                    ));
                }
            }
            GodotAIEvent::ActorLost { observer, target } => {
                if let Ok((mut spotted, _)) = ai_query.get_mut(*observer) {
                    let was_present = spotted.enemies.contains(target);
                    spotted.enemies.retain(|&e| e != *target);
                    if was_present {
                        crate::log(&format!(
                            "👻 ActorLost: {:?} lost sight of {:?} (removed from SpottedEnemies)",
                            observer, target
                        ));
                    }
                }
            }
        }
    }

    // Очищаем мёртвые entities из всех SpottedEnemies
    for (mut spotted, _) in ai_query.iter_mut() {
        let initial_count = spotted.enemies.len();
        spotted.enemies.retain(|&e| {
            potential_targets
                .get(e)
                .map(|h| h.is_alive())
                .unwrap_or(false) // Если entity despawned или нет Health — удаляем
        });

        let removed_count = initial_count - spotted.enemies.len();
        if removed_count > 0 {
            crate::log(&format!("AI: Removed {} dead/invalid targets from SpottedEnemies", removed_count));
        }
    }
}

/// Система: AI FSM transitions (event-driven)
///
/// Обновляет AIState на основе SpottedEnemies, health, stamina.
/// Порядок приоритетов:
/// 1. Retreat (если low health/stamina)
/// 2. Combat (если есть spotted enemies)
/// 3. Patrol (если никого не видим)
///
/// ADR-005: Использует StrategicPosition для AI decisions (не Godot Transform)
pub fn ai_fsm_transitions(
    mut ai_query: Query<(
        Entity,
        &mut AIState,
        &mut SpottedEnemies,
        &AIConfig,
        &Health,
        &Stamina,
        &crate::StrategicPosition,
        Option<&crate::combat::MeleeAttackState>, // Check if in attack animation
    )>,
    potential_targets: Query<&Health>, // Для проверки что target жив
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (entity, mut state, mut spotted, config, health, stamina, strategic_pos, melee_attack_state) in ai_query.iter_mut() {
        let stamina_percent = stamina.current / stamina.max;
        let health_percent = health.current as f32 / health.max as f32;

        // Проверяем нужно ли отступить
        // ⚠️ НЕ отступаем если в процессе атаки (MeleeAttackState active)!
        let should_retreat = melee_attack_state.is_none()
            && (stamina_percent < config.retreat_stamina_threshold
                || health_percent < config.retreat_health_threshold);

        let new_state = match state.as_ref() {
            AIState::Dead => {
                // Dead state — не переключаемся
                continue;
            }

            AIState::Idle => {
                // Idle → Patrol (начинаем патрулировать)
                crate::log(&format!("AI: {:?} Idle → Patrol", entity));
                AIState::Patrol {
                    next_direction_timer: config.patrol_direction_change_interval,
                    target_position: None, // Будет сгенерирована в ai_movement_from_state
                }
            }

            AIState::Patrol { next_direction_timer, target_position } => {
                // Если spotted enemy → Combat
                if let Some(&target) = spotted.enemies.first() {
                    crate::log(&format!("🔍 {:?} Patrol: spotted {} enemies, first = {:?}", entity, spotted.enemies.len(), target));
                    // Проверяем что target жив
                    if let Ok(target_health) = potential_targets.get(target) {
                        if target_health.is_alive() {
                            crate::log(&format!("⚔️ {:?} Patrol → Combat (target {:?})", entity, target));
                            AIState::Combat { target }
                        } else {
                            // Target мертв, продолжаем патруль
                            AIState::Patrol {
                                next_direction_timer: *next_direction_timer,
                                target_position: *target_position,
                            }
                        }
                    } else {
                        AIState::Patrol {
                            next_direction_timer: *next_direction_timer,
                            target_position: *target_position,
                        }
                    }
                } else {
                    // Продолжаем патруль, обновляем таймер
                    let new_timer = (*next_direction_timer - delta).max(0.0);

                    // Если таймер истёк → генерируем новую patrol точку (используем StrategicPosition)
                    let new_target = if new_timer <= 0.0 {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();

                        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                        let distance = 5.0 + rng.gen::<f32>() * 10.0; // 5-15м radius

                        // Генерируем от текущей strategic position
                        let current_world_pos = strategic_pos.to_world_position(0.5);
                        let offset = Vec3::new(angle.cos() * distance, 0.0, angle.sin() * distance);
                        let patrol_target = current_world_pos + offset;

                        // для теста генерируем точку всегда с -z от текущей позиции
                        // let patrol_target = Vec3::new(current_world_pos.x, current_world_pos.y, -current_world_pos.z);
                        Some(patrol_target)
                    } else {
                        *target_position
                    };

                    AIState::Patrol {
                        next_direction_timer: if new_timer <= 0.0 {
                            config.patrol_direction_change_interval
                        } else {
                            new_timer
                        },
                        target_position: new_target,
                    }
                }
            }

            AIState::Combat { target } => {
                // Проверяем retreat conditions
                if should_retreat {
                    crate::log(&format!("AI: {:?} Combat → Retreat (low hp/stamina)", entity));
                    AIState::Retreat {
                        timer: config.retreat_duration,
                        from_target: Some(*target),
                    }
                } else {
                    // Проверяем что target еще spotted и жив
                    let target_valid = spotted.enemies.contains(target)
                        && potential_targets
                            .get(*target)
                            .map(|h| h.is_alive())
                            .unwrap_or(false);

                    if !target_valid {
                        // Target потерян или мертв → ищем нового или патруль
                        crate::log(&format!("❌ {:?} Combat: target {:?} INVALID (in spotted: {}, alive: {})",
                            entity, target,
                            spotted.enemies.contains(target),
                            potential_targets.get(*target).map(|h| h.is_alive()).unwrap_or(false)
                        ));
                        if let Some(&new_target) = spotted.enemies.first() {
                            crate::log(&format!("🔄 {:?} Combat: target lost, switching to {:?}", entity, new_target));
                            AIState::Combat { target: new_target }
                        } else {
                            crate::log(&format!("🚶 {:?} Combat → Patrol (no targets in SpottedEnemies)", entity));
                            AIState::Patrol {
                                next_direction_timer: config.patrol_direction_change_interval,
                                target_position: None,
                            }
                        }
                    } else {
                        // Продолжаем бой
                        AIState::Combat { target: *target }
                    }
                }
            }

            AIState::Retreat { timer, from_target } => {
                let new_timer = (*timer - delta).max(0.0);

                if new_timer <= 0.0 {
                    // Retreat закончен — проверяем можем ли вернуться в Combat

                    // Приоритет 1: возвращаемся к from_target (даже если VisionCone потерял)
                    if let Some(target) = from_target {
                        // Проверяем что target всё ещё жив
                        if potential_targets.get(*target).map(|h| h.is_alive()).unwrap_or(false) {
                            // ✅ Добавляем from_target обратно в SpottedEnemies (VisionCone мог потерять во время retreat)
                            if !spotted.enemies.contains(target) {
                                spotted.enemies.push(*target);
                                crate::log(&format!("🔄 {:?} re-adding from_target {:?} to SpottedEnemies (lost during Retreat)", entity, target));
                            }
                            crate::log(&format!("AI: {:?} Retreat → Combat (return to from_target {:?})", entity, target));
                            AIState::Combat { target: *target }
                        } else {
                            // from_target мёртв — ищем другого spotted enemy
                            if let Some(&new_target) = spotted.enemies.first() {
                                crate::log(&format!("AI: {:?} Retreat → Combat (from_target dead, switching to {:?})", entity, new_target));
                                AIState::Combat { target: new_target }
                            } else {
                                // Никого нет → Patrol
                                crate::log(&format!("AI: {:?} Retreat → Patrol (no targets)", entity));
                                AIState::Patrol {
                                    next_direction_timer: config.patrol_direction_change_interval,
                                    target_position: None,
                                }
                            }
                        }
                    } else {
                        // Нет from_target — проверяем spotted enemies
                        if let Some(&target) = spotted.enemies.first() {
                            crate::log(&format!("AI: {:?} Retreat → Combat (spotted enemy)", entity));
                            AIState::Combat { target }
                        } else {
                            // Никого нет → Patrol
                            crate::log(&format!("AI: {:?} Retreat → Patrol (no targets)", entity));
                            AIState::Patrol {
                                next_direction_timer: config.patrol_direction_change_interval,
                                target_position: None,
                            }
                        }
                    }
                } else {
                    // Продолжаем retreat
                    AIState::Retreat {
                        timer: new_timer,
                        from_target: *from_target,
                    }
                }
            }
        };

        if *state != new_state {
            *state = new_state;
        }
    }
}
