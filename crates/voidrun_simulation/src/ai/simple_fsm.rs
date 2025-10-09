//! Simple FSM AI для combat
//!
//! Конечный автомат для NPC aggro behavior:
//! Idle → Aggro → Approach → Attack → Retreat → Idle
//!
//! Архитектура:
//! - FSM работает на 1Hz (или FixedUpdate для детерминизма в тестах)
//! - State transitions основаны на distance, health, stamina
//! - Attack state генерирует AttackStarted события

use bevy::prelude::*;
use crate::components::{Actor, Health, Stamina};
use crate::physics::MovementInput;
use crate::combat::{Attacker, AttackStarted, ATTACK_COST};

/// AI FSM состояния
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum AIState {
    /// Idle — ничего не делаем, ждем врага
    Idle,

    /// Aggro — заметили врага, решаем атаковать
    Aggro {
        target: Entity,
    },

    /// Approach — подходим к врагу в радиус атаки
    Approach {
        target: Entity,
    },

    /// Attack — атакуем (swing + cooldown)
    Attack {
        target: Entity,
    },

    /// Retreat — отступаем для восстановления stamina
    Retreat {
        /// Время отступления (секунды)
        timer: f32,
    },
}

impl Default for AIState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Параметры AI (aggressiveness, detection range, etc.)
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AIConfig {
    /// Радиус обнаружения врагов (метры)
    pub detection_range: f32,
    /// Stamina порог для отступления (percent)
    pub retreat_stamina_threshold: f32,
    /// Health порог для отступления (percent)
    pub retreat_health_threshold: f32,
    /// Время отступления (секунды)
    pub retreat_duration: f32,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            detection_range: 10.0,
            retreat_stamina_threshold: 0.3, // 30% stamina
            retreat_health_threshold: 0.2,  // 20% health
            retreat_duration: 2.0,
        }
    }
}

/// Система: AI FSM transitions
///
/// Обновляет AIState на основе окружения (target distance, health, stamina).
pub fn ai_fsm_transitions(
    mut ai_query: Query<(
        Entity,
        &Actor,
        &Transform,
        &mut AIState,
        &AIConfig,
        &Health,
        &Stamina,
        &Attacker,
    )>,
    potential_targets: Query<(Entity, &Actor, &Transform, &Health)>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    let ai_count = ai_query.iter().count();
    if ai_count > 0 {
        eprintln!("DEBUG: ai_fsm_transitions running, {} AI entities found", ai_count);
    }

    for (entity, actor, transform, mut state, config, health, stamina, attacker) in ai_query.iter_mut() {
        let new_state = match state.as_ref() {
            AIState::Idle => {
                // Ищем ближайшего живого врага (другой фракции)
                eprintln!("DEBUG: Entity {:?} (faction {}) searching for enemy at pos {:?}",
                    entity, actor.faction_id, transform.translation);

                if let Some(target) = find_nearest_enemy(
                    entity,
                    actor.faction_id,
                    transform,
                    &potential_targets,
                    config.detection_range,
                ) {
                    eprintln!("DEBUG: Entity {:?} found enemy {:?}, switching to Aggro", entity, target);
                    AIState::Aggro { target }
                } else {
                    eprintln!("DEBUG: Entity {:?} found NO enemies", entity);
                    AIState::Idle
                }
            }

            AIState::Aggro { target } => {
                // Проверяем что target еще жив и в радиусе
                if is_valid_target(*target, &potential_targets) {
                    AIState::Approach { target: *target }
                } else {
                    AIState::Idle
                }
            }

            AIState::Approach { target } => {
                // Проверяем distance до target
                if let Ok((_, _, target_transform, target_health)) = potential_targets.get(*target) {
                    if !target_health.is_alive() {
                        AIState::Idle
                    } else {

                        let distance = transform.translation.distance(target_transform.translation);

                        // Проверяем порог retreat
                        let stamina_percent = stamina.current / stamina.max;
                        let health_percent = health.current as f32 / health.max as f32;

                        if stamina_percent < config.retreat_stamina_threshold
                            || health_percent < config.retreat_health_threshold
                        {
                            AIState::Retreat {
                                timer: config.retreat_duration,
                            }
                        } else if distance < attacker.attack_radius {
                            // В радиусе атаки
                            AIState::Attack { target: *target }
                        } else {
                            // Продолжаем подходить
                            AIState::Approach { target: *target }
                        }
                    }
                } else {
                    AIState::Idle
                }
            }

            AIState::Attack { target } => {
                // Проверяем что target еще жив
                if let Ok((_, _, _, target_health)) = potential_targets.get(*target) {
                    if !target_health.is_alive() {
                        AIState::Idle
                    } else {

                        // Проверяем stamina/health для retreat
                        let stamina_percent = stamina.current / stamina.max;
                        let health_percent = health.current as f32 / health.max as f32;

                        if stamina_percent < config.retreat_stamina_threshold
                            || health_percent < config.retreat_health_threshold
                        {
                            AIState::Retreat {
                                timer: config.retreat_duration,
                            }
                        } else {
                            // Продолжаем атаковать
                            AIState::Attack { target: *target }
                        }
                    }
                } else {
                    AIState::Idle
                }
            }

            AIState::Retreat { timer } => {
                let new_timer = timer - delta;

                if new_timer <= 0.0 {
                    // Восстановились, возвращаемся к idle
                    AIState::Idle
                } else {
                    // Продолжаем отступать
                    AIState::Retreat { timer: new_timer }
                }
            }
        };

        *state = new_state;
    }
}

/// Система: AI movement от FSM state
///
/// Конвертирует AIState в MovementInput для physics системы.
pub fn ai_movement_from_state(
    mut ai_query: Query<(Entity, &Transform, &AIState, &mut MovementInput)>,
    targets: Query<(Entity, &Transform)>,
) {
    const MIN_DISTANCE: f32 = 0.5; // Минимальная дистанция между NPC

    for (_entity, transform, state, mut movement) in ai_query.iter_mut() {
        match state {
            AIState::Idle => {
                // Не двигаемся
                movement.direction = Vec3::ZERO;
            }

            AIState::Aggro { target } | AIState::Approach { target } => {
                // Двигаемся к target
                if let Ok((_, target_transform)) = targets.get(*target) {
                    let to_target = target_transform.translation - transform.translation;
                    let distance = to_target.length();

                    if distance > MIN_DISTANCE {
                        // Достаточно далеко — идём к цели
                        movement.direction = to_target.normalize_or_zero();
                    } else {
                        // Слишком близко — стоим на месте
                        movement.direction = Vec3::ZERO;
                    }
                } else {
                    movement.direction = Vec3::ZERO;
                }
            }

            AIState::Attack { .. } => {
                // Стоим на месте во время атаки
                movement.direction = Vec3::ZERO;
            }

            AIState::Retreat { .. } => {
                // Отступаем назад (reverse direction к ближайшему врагу)
                // Упрощенная версия: просто стоим на месте, восстанавливаем stamina
                movement.direction = Vec3::ZERO;
                // TODO: в будущем можно добавить pathfinding от врагов
            }
        }
    }
}

/// Система: AI attack execution
///
/// Генерирует AttackStarted события когда AI в Attack state и cooldown готов.
pub fn ai_attack_execution(
    mut ai_query: Query<(Entity, &Transform, &AIState, &mut Attacker, &Stamina)>,
    mut attack_events: EventWriter<AttackStarted>,
) {
    for (entity, transform, state, mut attacker, stamina) in ai_query.iter_mut() {
        if let AIState::Attack { target: _ } = state {
            // Проверяем cooldown и stamina
            if attacker.can_attack() && stamina.can_afford(ATTACK_COST) {
                // Генерируем AttackStarted событие
                attack_events.write(AttackStarted {
                    attacker: entity,
                    damage: attacker.base_damage,
                    radius: attacker.attack_radius,
                    offset: transform.forward() * 1.0, // 1m вперед
                });

                // Запускаем cooldown
                attacker.start_attack();

                // Debug
                // eprintln!("DEBUG: AI entity {:?} started attack", entity);
            }
        }
    }
}

/// Helper: найти ближайшего врага (другой фракции) в радиусе
fn find_nearest_enemy(
    self_entity: Entity,
    self_faction: u64,
    self_transform: &Transform,
    targets: &Query<(Entity, &Actor, &Transform, &Health)>,
    max_range: f32,
) -> Option<Entity> {
    let mut nearest: Option<(Entity, f32)> = None;

    for (target_entity, target_actor, target_transform, target_health) in targets.iter() {
        // Не атакуем себя
        if target_entity == self_entity {
            continue;
        }

        // Только враги (другая фракция)
        if target_actor.faction_id == self_faction {
            continue;
        }

        // Только живые targets
        if !target_health.is_alive() {
            continue;
        }

        let distance = self_transform.translation.distance(target_transform.translation);

        if distance <= max_range {
            if let Some((_, best_distance)) = nearest {
                if distance < best_distance {
                    nearest = Some((target_entity, distance));
                }
            } else {
                nearest = Some((target_entity, distance));
            }
        }
    }

    nearest.map(|(entity, _)| entity)
}

/// Helper: проверить что target еще валиден (жив)
fn is_valid_target(
    target: Entity,
    targets: &Query<(Entity, &Actor, &Transform, &Health)>,
) -> bool {
    if let Ok((_, _, _, health)) = targets.get(target) {
        health.is_alive()
    } else {
        false
    }
}

/// Система: простая collision resolution для NPC
///
/// Отталкивает NPC друг от друга если они слишком близко (< 0.8m).
/// Работает как замена физическим коллайдерам в headless режиме.
pub fn simple_collision_resolution(
    mut query: Query<(Entity, &mut Transform, &Actor), With<AIState>>,
) {
    const COLLISION_RADIUS: f32 = 0.8; // Минимальная дистанция
    const PUSH_STRENGTH: f32 = 0.1;     // Сила отталкивания

    let mut entities_positions: Vec<(Entity, Vec3)> = query.iter()
        .map(|(e, t, _)| (e, t.translation))
        .collect();

    for (entity, mut transform, _) in query.iter_mut() {
        let mut push = Vec3::ZERO;

        for (other_entity, other_pos) in &entities_positions {
            if *other_entity == entity {
                continue;
            }

            let to_other = *other_pos - transform.translation;
            let distance = to_other.length();

            if distance < COLLISION_RADIUS && distance > 0.01 {
                // Отталкиваемся от other
                let push_dir = -to_other.normalize();
                let push_amount = (COLLISION_RADIUS - distance) * PUSH_STRENGTH;
                push += push_dir * push_amount;
            }
        }

        transform.translation += push;
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
        assert_eq!(config.detection_range, 10.0);
        assert_eq!(config.retreat_stamina_threshold, 0.3);
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
