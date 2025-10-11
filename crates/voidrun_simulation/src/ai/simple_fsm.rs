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
use crate::combat::Attacker;
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
pub fn update_spotted_enemies(
    mut ai_query: Query<&mut SpottedEnemies>,
    mut ai_events: EventReader<GodotAIEvent>,
    potential_targets: Query<&Health>, // Для проверки что target жив
) {
    for event in ai_events.read() {
        match event {
            GodotAIEvent::ActorSpotted { observer, target } => {
                if let Ok(mut spotted) = ai_query.get_mut(*observer) {
                    if !spotted.enemies.contains(target) {
                        spotted.enemies.push(*target);
                    }
                }
            }
            GodotAIEvent::ActorLost { observer, target } => {
                if let Ok(mut spotted) = ai_query.get_mut(*observer) {
                    spotted.enemies.retain(|&e| e != *target);
                }
            }
        }
    }

    // Очищаем мёртвые entities из всех SpottedEnemies
    for mut spotted in ai_query.iter_mut() {
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
pub fn ai_fsm_transitions(
    mut ai_query: Query<(
        Entity,
        &mut AIState,
        &SpottedEnemies,
        &AIConfig,
        &Health,
        &Stamina,
        &Transform,
    )>,
    potential_targets: Query<&Health>, // Для проверки что target жив
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (entity, mut state, spotted, config, health, stamina, transform) in ai_query.iter_mut() {
        let stamina_percent = stamina.current / stamina.max;
        let health_percent = health.current as f32 / health.max as f32;

        // Проверяем нужно ли отступить
        let should_retreat = stamina_percent < config.retreat_stamina_threshold
            || health_percent < config.retreat_health_threshold;

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
                    // Проверяем что target жив
                    if let Ok(target_health) = potential_targets.get(target) {
                        if target_health.is_alive() {
                            crate::log(&format!("AI: {:?} Patrol → Combat (target {:?})", entity, target));
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

                    // Если таймер истёк → генерируем новую patrol точку
                    let new_target = if new_timer <= 0.0 {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();

                        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                        let distance = 5.0 + rng.gen::<f32>() * 100.0; // 5-15м radius
                        let offset = Vec3::new(angle.cos() * distance, 0.0, angle.sin() * distance);
                        let patrol_target = transform.translation + offset;

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
                        if let Some(&new_target) = spotted.enemies.first() {
                            crate::log(&format!("AI: {:?} Combat: target lost, switching to {:?}", entity, new_target));
                            AIState::Combat { target: new_target }
                        } else {
                            crate::log(&format!("AI: {:?} Combat → Patrol (no targets)", entity));
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
                    // Retreat закончен
                    if let Some(&target) = spotted.enemies.first() {
                        // Есть spotted enemy → обратно в Combat
                        crate::log(&format!("AI: {:?} Retreat → Combat", entity));
                        AIState::Combat { target }
                    } else {
                        // Никого нет → Patrol
                        crate::log(&format!("AI: {:?} Retreat → Patrol", entity));
                        AIState::Patrol {
                            next_direction_timer: config.patrol_direction_change_interval,
                            target_position: None,
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
pub fn ai_movement_from_state(
    mut ai_query: Query<(&AIState, &mut MovementCommand, &Transform)>,
    targets_query: Query<&Transform>,
) {
    for (state, mut command, transform) in ai_query.iter_mut() {
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
                // Двигаемся к target (каждый frame обновляем, target движется!)
                if let Ok(target_transform) = targets_query.get(*target) {
                    let target_pos = target_transform.translation;
                    // Combat: target двигается → обновляем каждый frame
                    // НЕ проверяем matches, потому что позиция меняется
                    *command = MovementCommand::MoveToPosition {
                        target: target_pos,
                    };
                } else {
                    if !matches!(*command, MovementCommand::Idle) {
                        *command = MovementCommand::Idle;
                    }
                }
            }

            AIState::Retreat { from_target, .. } => {
                // Отступаем от target (противоположное направление)
                if let Some(target_entity) = from_target {
                    if let Ok(target_transform) = targets_query.get(*target_entity) {
                        let to_target = target_transform.translation - transform.translation;
                        let retreat_direction = -to_target.normalize();
                        let retreat_position = transform.translation + retreat_direction * 5.0; // 5 метров назад

                        // Retreat: target двигается → обновляем каждый frame
                        *command = MovementCommand::MoveToPosition {
                            target: retreat_position,
                        };
                    } else {
                        if !matches!(*command, MovementCommand::Idle) {
                            *command = MovementCommand::Idle;
                        }
                    }
                } else {
                    if !matches!(*command, MovementCommand::Idle) {
                        *command = MovementCommand::Idle;
                    }
                }
            }
        }
    }
}

/// Система: AI attack execution
///
/// Генерирует атаки когда в Combat state и target в радиусе.
pub fn ai_attack_execution(
    mut ai_query: Query<(&AIState, &Transform, &mut Attacker, &Stamina)>,
    targets_query: Query<&Transform>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (state, transform, mut attacker, stamina) in ai_query.iter_mut() {
        // Обновляем cooldown
        if attacker.cooldown_timer > 0.0 {
            attacker.cooldown_timer -= delta;
        }

        // Атакуем только в Combat state
        if let AIState::Combat { target } = state {
            if let Ok(target_transform) = targets_query.get(*target) {
                let distance = transform.translation.distance(target_transform.translation);

                // Проверяем: в радиусе, cooldown готов, есть stamina
                const ATTACK_COST: f32 = 20.0;
                if distance <= attacker.attack_radius
                    && attacker.cooldown_timer <= 0.0
                    && stamina.current >= ATTACK_COST
                {
                    // Атака происходит через старую систему (combat systems обрабатывают)
                    // Просто сбрасываем cooldown
                    attacker.cooldown_timer = attacker.attack_cooldown;

                    crate::log(&format!("AI: attacking target {:?}", target));
                }
            }
        }
    }
}

/// Система: collision resolution (отталкивание NPC друг от друга)
///
/// Предотвращает стэкинг actors на одной точке.
pub fn simple_collision_resolution(
    mut actors: Query<(&mut Transform, Entity), With<Actor>>,
) {
    let positions: Vec<(Entity, Vec3)> = actors
        .iter()
        .map(|(t, e)| (e, t.translation))
        .collect();

    for (mut transform, entity) in actors.iter_mut() {
        let mut push = Vec3::ZERO;

        for &(other_entity, other_pos) in &positions {
            if other_entity == entity {
                continue;
            }

            let diff = transform.translation - other_pos;
            let distance = diff.length();

            // Минимальная дистанция между actors
            const MIN_DISTANCE: f32 = 1.0;

            if distance < MIN_DISTANCE && distance > 0.001 {
                let push_force = (MIN_DISTANCE - distance) / MIN_DISTANCE;
                push += diff.normalize() * push_force * 0.1;
            }
        }

        transform.translation += push;
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
