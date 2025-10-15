//! Simple FSM AI для combat (Godot-driven architecture)
//!
//! ECS ответственность:
//! - State machine logic (Idle → Patrol → Combat)
//! - Decisions (когда атаковать, куда патрулировать)
//! - Target tracking (кто кого видит)
//!
//! Godot ответственность:
//! - VisionCone (Area3D) → GodotAIEvent (ActorSpotted/ActorLost)
//! - Pathfinding (NavigationAgent3D)
//! - Movement execution (CharacterBody3D)
//!
//! Architecture: ADR-005 (Godot authoritative), event-driven AI

use bevy::prelude::*;
use crate::components::{Actor, Health, Stamina, MovementCommand};
use crate::combat::WeaponStats;
use crate::ai::GodotAIEvent;

/// AI FSM состояния (event-driven)
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum AIState {
    /// Idle — начальное состояние после спавна
    Idle,

    /// Patrol — случайное движение в поисках врагов
    Patrol {
        /// Время до следующей смены направления
        next_direction_timer: f32,
        /// Текущая target позиция патруля (генерируется случайно)
        target_position: Option<Vec3>,
    },

    /// Combat — бой с обнаруженным врагом
    Combat {
        target: Entity,
    },

    /// Retreat — отступление для восстановления
    Retreat {
        /// Время отступления (секунды)
        timer: f32,
        /// От кого отступаем (опционально)
        from_target: Option<Entity>,
    },

    /// Dead — актёр мертв (HP == 0), AI отключен
    Dead,
}

impl Default for AIState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Component: tracking spotted enemies (от GodotAIEvent)
///
/// Обновляется через ActorSpotted/ActorLost events.
/// AI использует для выбора target из множества spotted врагов.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct SpottedEnemies {
    pub enemies: Vec<Entity>,
}

/// Параметры AI
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AIConfig {
    /// Stamina порог для отступления (percent)
    pub retreat_stamina_threshold: f32,
    /// Health порог для отступления (percent)
    pub retreat_health_threshold: f32,
    /// Время отступления (секунды)
    pub retreat_duration: f32,
    /// Patrol: время между сменой направления (секунды)
    pub patrol_direction_change_interval: f32,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            retreat_stamina_threshold: 0.3, // 30% stamina
            retreat_health_threshold: 0.2,  // 20% health
            retreat_duration: 2.0,
            patrol_direction_change_interval: 10.0, // Каждые 10 сек новое направление (было 3 сек)
        }
    }
}

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

/// Система: AI movement from state
///
/// Конвертирует AIState → MovementCommand для Godot.
/// ADR-005: Используем StrategicPosition для AI decisions
pub fn ai_movement_from_state(
    mut ai_query: Query<(&AIState, &mut MovementCommand, &crate::StrategicPosition)>,
    targets_query: Query<&crate::StrategicPosition>,
) {
    for (state, mut command, strategic_pos) in ai_query.iter_mut() {
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

/// System: обработка смерти → переключение AI в Dead state
///
/// При HP == 0 отключаем AI (Dead state) чтобы мертвые не стреляли/двигались
pub fn handle_actor_death(
    mut actors: Query<(&crate::Health, &mut AIState), Changed<crate::Health>>,
) {
    for (health, mut state) in actors.iter_mut() {
        if health.current == 0 && !matches!(*state, AIState::Dead) {
            *state = AIState::Dead;
            crate::log("Actor died → AI disabled (Dead state)");
        }
    }
}

/// System: AI реакция на полученный урон
///
/// Если актора ударили, он автоматически:
/// - Добавляет атакующего в SpottedEnemies (если враг)
/// - Разворачивается к атакующему (через MovementCommand::FollowEntity)
/// - FSM перейдёт в Combat на следующем тике через ai_fsm_transitions
///
/// Это обеспечивает естественную реакцию "ударили в спину → развернулся и дерёшься"
pub fn react_to_damage(
    mut damage_events: EventReader<crate::combat::DamageDealt>,
    mut actors: Query<(&Actor, &mut SpottedEnemies, &mut MovementCommand)>,
    attackers: Query<&Actor>,
) {
    for damage_event in damage_events.read() {
        // Получаем victim actor
        let Ok((victim_actor, mut spotted_enemies, mut command)) = actors.get_mut(damage_event.target) else {
            continue;
        };

        // Получаем attacker actor
        let Ok(attacker_actor) = attackers.get(damage_event.attacker) else {
            continue;
        };

        // Проверяем фракции: реагируем только на врагов
        if victim_actor.faction_id == attacker_actor.faction_id {
            // Friendly fire — игнорируем (или можно добавить другую логику)
            continue;
        }

        // Добавляем атакующего в SpottedEnemies (если ещё не там)
        if !spotted_enemies.enemies.contains(&damage_event.attacker) {
            spotted_enemies.enemies.push(damage_event.attacker);
            crate::log(&format!(
                "⚠️ {:?} damaged by {:?} → added to SpottedEnemies",
                damage_event.target, damage_event.attacker
            ));
        }

        // Разворачиваемся к атакующему (FollowEntity даст NavigationAgent3D развернуться)
        *command = MovementCommand::FollowEntity {
            target: damage_event.attacker,
        };

        crate::log(&format!(
            "🔥 {:?} hit by {:?} → following attacker",
            damage_event.target, damage_event.attacker
        ));
    }
}

/// System: AI реакция на звук выстрела
///
/// Архитектура:
/// - Слушает WeaponFired события (содержат shooter_position + hearing_range)
/// - Проверяет расстояние через StrategicPosition (chunk-aware distance)
/// - Генерирует ActorSpotted event для имитации "услышал стрелявшего"
/// - Устанавливает MovementCommand в сторону выстрела с разбросом 3м
///
/// Логика:
/// - Все актёры в радиусе слышат выстрел (союзники, враги, нейтралы)
/// - Skip: сам стрелявший, актёры уже в Combat (сосредоточены на своей цели)
/// - Радиус слышимости зависит от оружия (pistol ~25м, rifle ~40м, sniper ~60м)
pub fn ai_react_to_gunfire(
    mut gunfire_events: EventReader<crate::combat::WeaponFired>,
    mut actors: Query<(Entity, &Actor, &crate::StrategicPosition, &AIState, &mut MovementCommand)>,
    mut spotted_events: EventWriter<GodotAIEvent>,
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for fire_event in gunfire_events.read() {
        // Конвертируем world position → StrategicPosition для distance check
        let shooter_strategic = crate::StrategicPosition::from_world_position(
            fire_event.shooter_position
        );

        for (listener_entity, _listener_actor, listener_pos, ai_state, mut command) in actors.iter_mut() {
            // Skip: сам стрелявший
            if listener_entity == fire_event.shooter {
                continue;
            }

            // Skip: уже в Combat (сосредоточен на своей цели, не отвлекается)
            if matches!(ai_state, AIState::Combat { .. }) {
                continue;
            }

            // Проверка расстояния (chunk-aware distance через world positions)
            let listener_world_pos = listener_pos.to_world_position(0.5);
            let shooter_world_pos = shooter_strategic.to_world_position(0.5);
            let distance = listener_world_pos.distance(shooter_world_pos);

            if distance > fire_event.hearing_range {
                continue;
            }

            // ✅ Услышал выстрел!
            crate::log(&format!(
                "🔊 Entity {:?} heard gunfire from {:?} at distance {:.1}m (range: {:.1}m)",
                listener_entity, fire_event.shooter, distance, fire_event.hearing_range
            ));

            // Генерируем ActorSpotted (имитация "услышал и заметил стрелявшего")
            spotted_events.write(GodotAIEvent::ActorSpotted {
                observer: listener_entity,
                target: fire_event.shooter,
            });

            // Идём в сторону выстрела с разбросом 3м (неуверенность в точной позиции)
            let random_offset = Vec3::new(
                rng.gen_range(-1.0..1.0), // -1..1
                0.0,
                rng.gen_range(-1.0..1.0),
            ) * 3.0; // 3м разброс

            let investigate_pos = fire_event.shooter_position + random_offset;
            *command = MovementCommand::MoveToPosition {
                target: investigate_pos,
            };

            crate::log(&format!(
                "  → Entity {:?} moving to investigate gunfire at {:?}",
                listener_entity, investigate_pos
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_state_default() {
        let state = AIState::default();
        assert!(matches!(state, AIState::Idle));
    }

    #[test]
    fn test_ai_config_default() {
        let config = AIConfig::default();
        assert_eq!(config.retreat_stamina_threshold, 0.3);
        assert_eq!(config.retreat_health_threshold, 0.2);
        assert_eq!(config.retreat_duration, 2.0);
        assert_eq!(config.patrol_direction_change_interval, 3.0);
    }

    #[test]
    fn test_retreat_timer_logic() {
        let mut timer = 2.0;
        let delta = 0.5;

        timer -= delta;
        assert_eq!(timer, 1.5);

        timer -= delta;
        assert_eq!(timer, 1.0);

        timer -= delta;
        assert_eq!(timer, 0.5);

        timer -= delta;
        assert_eq!(timer, 0.0);
        assert!(timer <= 0.0); // Retreat завершен
    }

    #[test]
    fn test_find_nearest_target_logic() {
        // Логика поиска ближайшего (без App)
        let self_pos = Vec3::new(0.0, 0.0, 0.0);
        let target1_pos = Vec3::new(5.0, 0.0, 0.0); // distance = 5.0
        let target2_pos = Vec3::new(3.0, 0.0, 0.0); // distance = 3.0 (ближе)
        let target3_pos = Vec3::new(15.0, 0.0, 0.0); // distance = 15.0 (вне радиуса)

        let max_range = 10.0;

        assert!(self_pos.distance(target1_pos) <= max_range);
        assert!(self_pos.distance(target2_pos) <= max_range);
        assert!(self_pos.distance(target2_pos) < self_pos.distance(target1_pos)); // target2 ближе
        assert!(self_pos.distance(target3_pos) > max_range);
    }
}
