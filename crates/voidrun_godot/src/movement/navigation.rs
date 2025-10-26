//! Navigation systems (pathfinding, follow entity targets).
//!
//! Системы:
//! - `update_follow_entity_targets_main_thread`: Обновление target_position в NavigationAgent3D для FollowEntity команд
//! - `apply_navigation_velocity_main_thread`: Применение NavigationAgent3D → CharacterBody3D движение

use super::commands::adjust_distance_for_los;
use crate::shared::VisualRegistry;
use bevy::prelude::*;
use godot::classes::{CharacterBody3D, NavigationAgent3D, Node};
use godot::prelude::*;
use voidrun_simulation::{MovementCommand, NavigationState};

/// Helper: логирование каждые 30 кадров (уменьшает спам)
fn log_every_30_frames(message: &str) {
    static mut FRAME_COUNTER: u32 = 0;
    unsafe {
        FRAME_COUNTER += 1;
        if FRAME_COUNTER % 30 == 0 {
            voidrun_simulation::logger::log(message);
        }
    }
}

/// Обновление follow entity targets для NavigationAgent3D
///
/// Система обрабатывает MovementCommand::FollowEntity:
/// - Обновляет target_position в NavigationAgent3D каждый кадр (target двигается!)
/// - Устанавливает target_desired_distance в зависимости от оружия:
///   * Melee weapon: attack_radius БЕЗ буфера (подходим вплотную)
///   * Ranged weapon: range - буфер безопасности (держим дистанцию)
/// - Корректирует дистанцию через adjust_distance_for_los при блокировке LOS
///
/// ADR-004: Changed<MovementCommand> → Godot NavigationAgent
/// Main thread only (Godot API)
pub fn update_follow_entity_targets_main_thread(
    mut query: Query<(
        Entity,
        &MovementCommand,
        &mut NavigationState,
        Option<&voidrun_simulation::combat::WeaponStats>,
    )>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::shared::SceneRoot>,
) {
    for (entity, command, mut nav_state, weapon_opt) in query.iter_mut() {
        let MovementCommand::FollowEntity { target } = command else {
            continue;
        };

        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let Some(mut nav_agent) =
            actor_node.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D")
        else {
            continue;
        };

        let Some(target_node) = visuals.visuals.get(target) else {
            continue;
        };

        // Обновляем target position каждый кадр (target двигается!)
        let target_pos = target_node.get_position();
        nav_agent.set_target_position(target_pos);

        // Дистанция остановки зависит от типа оружия:
        // - Melee (attack_radius > 0): подходим вплотную (БЕЗ буфера)
        // - Ranged (range > 0): держим дистанцию (с буфером безопасности)
        const RANGED_STOP_BUFFER: f32 = 2.0; // Буфер для ranged оружия

        let (base_distance, weapon_type) = if let Some(weapon) = weapon_opt {
            if weapon.attack_radius > 0.0 {
                // Melee weapon — используем attack_radius БЕЗ буфера
                (weapon.attack_radius, "melee")
            } else {
                // Ranged weapon — используем range с буфером
                ((weapon.range - RANGED_STOP_BUFFER).max(0.5), "ranged")
            }
        } else {
            // Fallback для акторов без оружия
            (15.0, "default")
        };

        // Проверяем LOS и корректируем distance если нужно
        // Передаём current_follow_distance из NavigationState (stateful iteration)
        let adjusted_distance = adjust_distance_for_los(
            entity,
            *target,
            nav_state.current_follow_distance,
            base_distance,
            &visuals,
            &scene_root,
        );

        // Сохраняем adjusted distance обратно в NavigationState
        nav_state.current_follow_distance = Some(adjusted_distance);

        nav_agent.set_target_desired_distance(adjusted_distance);
    }
}

/// Применение NavigationAgent3D → CharacterBody3D движение
///
/// Берём get_next_path_position() от NavigationAgent и применяем velocity.
/// Avoidance отключён — простой pathfinding для single-player game.
/// ADR-005: Отправляем GodotTransformEvent::PositionChanged после move_and_slide
///
/// NavigationState используется для one-time PositionChanged event (избегаем спама).
pub fn apply_navigation_velocity_main_thread(
    mut query: Query<
        (
            Entity,
            &mut voidrun_simulation::ai::AIState,
            &mut NavigationState,
        ),
        With<voidrun_simulation::Actor>,
    >,
    visuals: NonSend<VisualRegistry>,
    mut transform_events: EventWriter<voidrun_simulation::ai::GodotTransformEvent>,
) {
    const MOVE_SPEED: f32 = 5.0; // метры в секунду

    for (entity, mut ai_state, mut nav_state) in query.iter_mut() {
        // actor_node теперь САМ CharacterBody3D (root node из TSCN)
        let Some(actor_node) = visuals.visuals.get(&entity).cloned() else {
            continue;
        };

        // Cast root node к CharacterBody3D
        let mut body = actor_node.cast::<CharacterBody3D>();

        let Some(mut nav_agent) = body.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D")
        else {
            continue;
        };

        // КРИТИЧНО: Проверяем что путь валиден (NavigationAgent имеет цель и рассчитал путь)
        // is_target_reachable() = false если путь не найден или цель не установлена
        if !nav_agent.is_target_reachable() {
            // Нет валидного пути — стоим на месте
            nav_agent.set_velocity(Vector3::ZERO);
            body.set_velocity(Vector3::ZERO);
            // TODO: send event чтобы AI:State сгенерировал новый MovementCommand
            if nav_state.can_reach_target {
                nav_state.can_reach_target = false;
                *ai_state = voidrun_simulation::ai::AIState::Idle;
            }
            continue;
        }
        nav_state.can_reach_target = true;
        // Проверяем достигли ли цели (как enemy.gd:36)
        if nav_agent.is_target_reached() {
            log_every_30_frames(&format!("[Movement] target reached"));
            nav_agent.set_velocity(Vector3::ZERO);
            body.set_velocity(Vector3::ZERO);

            // ✅ Отправляем PositionChanged event только ОДИН РАЗ при достижении
            // Используем NavigationState.is_target_reached флаг (избегаем спама)
            if !nav_state.is_target_reached {
                nav_state.is_target_reached = true;
                let current_pos = body.get_position();
                transform_events.write(
                    voidrun_simulation::ai::GodotTransformEvent::PositionChanged {
                        entity,
                        position: Vec3::new(current_pos.x, current_pos.y, current_pos.z),
                    },
                );
                voidrun_simulation::logger::log(&format!(
                    "Entity {:?}: navigation target reached (one-time event sent)",
                    entity
                ));
            }
            continue;
        }

        // Вычисляем направление к следующей waypoint (enemy.gd:73-76)
        let next_pos = nav_agent.get_next_path_position();
        let current_pos = body.get_global_position();
        let target_pos = nav_agent.get_target_position();

        // Диагностика: логируем target, reachable, next waypoint
        log_every_30_frames(&format!(
            "[Movement] target: {:?}, reachable: {}, current: {:?} → next: {:?} (dist: {:.2}m)",
            target_pos,
            nav_agent.is_target_reachable(),
            current_pos,
            next_pos,
            (next_pos - current_pos).length()
        ));
        let diff = next_pos - current_pos;
        // Проверяем что вектор не нулевой ДО normalized()
        if diff.length() < 0.01 {
            nav_agent.set_velocity(Vector3::ZERO);
            body.set_velocity(Vector3::ZERO);
            continue;
        }

        let local_direction = diff.normalized();

        // Вычисляем desired_velocity в м/с (как enemy.gd line 37)
        let desired_velocity = Vector3::new(
            local_direction.x * MOVE_SPEED,
            0.0, // NavigationAgent работает в XZ плоскости (Y=0)
            local_direction.z * MOVE_SPEED,
        );

        // Передаём desired_velocity в AvoidanceReceiver (для debug логирования)
        if let Some(mut avoidance_receiver) = body.try_get_node_as::<Node>("AvoidanceReceiver") {
            // Устанавливаем desired_velocity property (используется в on_velocity_computed для diff)
            avoidance_receiver.set("desired_velocity", &desired_velocity.to_variant());
        }

        // КРИТИЧНО: Отправляем desired_velocity в NavigationAgent3D для avoidance расчёта
        // NavigationServer3D рассчитает safe_velocity с учётом других агентов
        // и вызовет signal velocity_computed → AvoidanceReceiver → SafeVelocityComputed event
        nav_agent.set_velocity(desired_velocity);

        // НЕ вызываем body.set_velocity() здесь!
        // apply_safe_velocity_system прочитает SafeVelocityComputed event и применит safe_velocity
    }
}

