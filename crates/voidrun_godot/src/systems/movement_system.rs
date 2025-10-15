//! Movement system — обработка MovementCommand → NavigationAgent3D
//!
//! Architecture: ADR-004 (Changed<MovementCommand> → Godot NavigationAgent)
//! Main thread only (Godot API)
//!
//! ВАЖНО: NavigationAgent3D паттерн (упрощённый, без avoidance):
//! 1. Устанавливаем target_position при изменении MovementCommand
//! 2. Каждый frame: берём get_next_path_position() от NavigationAgent
//! 3. Вычисляем направление к waypoint
//! 4. Применяем velocity к CharacterBody3D напрямую (без avoidance)
//!
//! ПОЧЕМУ НЕ velocity_computed callback:
//! - Требует avoidance_enabled = true
//! - Сложная интеграция с ECS (нужен wrapper class или untyped connect)
//! - Для single-player достаточно простого pathfinding без obstacle avoidance

use crate::systems::visual_registry::VisualRegistry;
use bevy::prelude::*;
use godot::classes::{
    BoxMesh, CharacterBody3D, Material, MeshInstance3D, NavigationAgent3D, StandardMaterial3D,
};
use godot::prelude::*;
use voidrun_simulation::{MovementCommand, NavigationState};

/// Debug: создаёт красный box marker в указанной позиции
fn spawn_debug_marker(position: Vector3, scene_root: &mut Gd<Node>) {
    let mut marker = MeshInstance3D::new_alloc();

    // Красный box mesh
    let mut box_mesh = BoxMesh::new_gd();
    box_mesh.set_size(Vector3::new(0.5, 0.5, 0.5));
    marker.set_mesh(&box_mesh.upcast::<BoxMesh>());

    // Красный материал
    let mut material = StandardMaterial3D::new_gd();
    material.set_albedo(Color::from_rgb(1.0, 0.0, 0.0)); // Ярко-красный
    marker.set_surface_override_material(0, &material.upcast::<Material>());

    marker.set_position(position);
    scene_root.add_child(&marker.upcast::<Node>());
}

/// Обработка MovementCommand → NavigationAgent3D target
///
/// КРИТИЧНО: set_target_position() вызывается при Changed<MovementCommand>
/// NavigationState.is_target_reached сбрасывается при новом MovementCommand.
pub fn process_movement_commands_main_thread(
    mut query: Query<
        (
            Entity,
            &MovementCommand,
            &mut NavigationState,
            Option<&voidrun_simulation::combat::WeaponStats>,
        ),
        Changed<MovementCommand>,
    >,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, command, mut nav_state, weapon_opt) in query.iter_mut() {
        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let Some(mut nav_agent) =
            actor_node.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D")
        else {
            continue;
        };

        match command {
            MovementCommand::Idle => {
                // Idle — НЕ сбрасываем флаг (сохраняем историю последнего движения)
                nav_agent.set_target_position(actor_node.get_position());
            }
            MovementCommand::MoveToPosition { target } => {
                // Новая цель → сбрасываем флаг (нужно заново отправить event при достижении)
                nav_state.is_target_reached = false;

                let target_vec = Vector3::new(target.x, target.y, target.z);
                nav_agent.set_target_position(target_vec);
                nav_agent.set_target_desired_distance(0.1);

                voidrun_simulation::log(&format!(
                    "Entity {:?}: new MoveToPosition target {:?}, reset reached flag",
                    entity, target
                ));
            }
            MovementCommand::FollowEntity { target } => {
                // Следование за entity → сбрасываем флаг при смене target ИЛИ превышении дистанции
                // TODO: Вариант B (distance threshold) — требует query target entity position
                if nav_state.last_follow_target != Some(*target) {
                    nav_state.is_target_reached = false;
                    nav_state.last_follow_target = Some(*target);

                    voidrun_simulation::log(&format!(
                        "Entity {:?}: new FollowEntity target {:?}, reset reached flag",
                        entity, target
                    ));
                }

                let Some(target_node) = visuals.visuals.get(&target) else {
                    continue;
                };

                let target_pos = target_node.get_position();
                nav_agent.set_target_position(target_pos);

                // Дистанция остановки зависит от типа оружия:
                // - Melee (attack_radius > 0): подходим вплотную (БЕЗ буфера)
                // - Ranged (range > 0): держим дистанцию (с буфером безопасности)
                const RANGED_STOP_BUFFER: f32 = 2.0; // Буфер для ranged оружия

                let (stop_distance, weapon_type) = if let Some(weapon) = weapon_opt {
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

                nav_agent.set_target_desired_distance(stop_distance);

                voidrun_simulation::log(&format!(
                    "Entity {:?}: FollowEntity target {:?} (stop at {:.1}m, type: {})",
                    entity, target_pos, stop_distance, weapon_type
                ));
            }
            MovementCommand::RetreatFrom { target } => {
                // RetreatFrom — не используем NavigationAgent (прямое управление velocity)
                // Просто сбрасываем флаг для consistency
                nav_state.is_target_reached = false;

                // Устанавливаем NavigationAgent target на текущую позицию (отключаем pathfinding)
                nav_agent.set_target_position(actor_node.get_position());

                voidrun_simulation::log(&format!(
                    "Entity {:?}: RetreatFrom {:?} (direct velocity control)",
                    entity, target
                ));
            }
            MovementCommand::Stop => {
                // Stop — НЕ сбрасываем флаг (останавливаемся, но сохраняем историю)
                nav_agent.set_target_position(actor_node.get_position());
            }
        }
    }
}

/// Apply RetreatFrom velocity (backpedal while facing target)
///
/// Тактическое отступление:
/// - Двигаемся НАЗАД от target (retreat direction)
/// - Смотрим НА target (look_at)
/// - Прямое управление velocity (NavigationAgent не используется)
pub fn apply_retreat_velocity_main_thread(
    query: Query<(Entity, &MovementCommand)>,
    visuals: NonSend<VisualRegistry>,
    mut transform_events: EventWriter<voidrun_simulation::ai::GodotTransformEvent>,
) {
    const RETREAT_SPEED: f32 = 3.0; // Отступаем медленнее чем движемся вперёд

    for (entity, command) in query.iter() {
        let MovementCommand::RetreatFrom { target } = command else {
            continue;
        };

        // Get actor node
        let Some(actor_node) = visuals.visuals.get(&entity).cloned() else {
            continue;
        };
        let mut body = actor_node.cast::<CharacterBody3D>();

        // Get target node
        let Some(target_node) = visuals.visuals.get(target) else {
            continue;
        };

        let current_pos = body.get_global_position();
        let target_pos = target_node.get_global_position();

        // Вектор ОТ target (direction to retreat)
        let to_target = target_pos - current_pos;
        let retreat_direction = -to_target.normalized();

        // Velocity: двигаемся НАЗАД
        let velocity = Vector3::new(
            retreat_direction.x * RETREAT_SPEED,
            body.get_velocity().y, // Сохраняем Y (гравитация)
            retreat_direction.z * RETREAT_SPEED,
        );

        // Rotation: смотрим НА target (не в направлении движения!)
        let look_at_pos = Vector3::new(target_pos.x, body.get_position().y, target_pos.z);
        body.look_at(look_at_pos);

        // Применяем velocity
        body.set_velocity(velocity);
        body.move_and_slide();

        // ✅ Send PositionChanged event EVERY FRAME during retreat
        let new_pos = body.get_position();
        transform_events.write(
            voidrun_simulation::ai::GodotTransformEvent::PositionChanged {
                entity,
                position: Vec3::new(new_pos.x, new_pos.y, new_pos.z),
            },
        );
    }
}

/// Update FollowEntity navigation targets every frame
///
/// FollowEntity требует постоянного обновления target_position (target движется).
/// Эта система работает КАЖДЫЙ КАДР (без Changed<> фильтра).
pub fn update_follow_entity_targets_main_thread(
    query: Query<(Entity, &MovementCommand)>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, command) in query.iter() {
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
    const MOVE_SPEED: f32 = 10.0; // метры в секунду

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
                voidrun_simulation::log(&format!(
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

/// Применение safe_velocity от NavigationAgent3D avoidance
///
/// Flow:
/// 1. apply_navigation_velocity вызвал nav_agent.set_velocity(desired_velocity)
/// 2. NavigationServer3D рассчитал safe_velocity с avoidance
/// 3. Signal velocity_computed → AvoidanceReceiver → SafeVelocityComputed event
/// 4. Эта система читает event и применяет safe_velocity к CharacterBody3D
///
/// НОВОЕ: Плавный поворот с динамическим замедлением velocity
/// - Скорость поворота ФИКСИРОВАННАЯ (ROTATION_SPEED рад/сек независимо от угла)
/// - Velocity масштабируется косинусом угла (замедление при повороте)
/// - Текущее направление берётся из Godot transform (не из ECS компонента)
/// - Использует slerp formula для плавной интерполяции направления
///
/// КРИТИЧНО: Запускается ПОСЛЕ apply_navigation_velocity (order matters!)
pub fn apply_safe_velocity_system(
    mut events: EventReader<crate::events::SafeVelocityComputed>,
    ai_query: Query<&voidrun_simulation::ai::AIState>,
    visuals: NonSend<VisualRegistry>,
    time: Res<Time>,
) {
    use godot::classes::CharacterBody3D;

    // Параметры плавного поворота
    const ROTATION_SPEED: f32 = 10.0; // рад/сек (фиксированная скорость)
    const VELOCITY_ANGLE_FACTOR: f32 = 1.0; // степень влияния угла на скорость (1.0 = линейное)
    const MIN_VELOCITY_SCALE: f32 = 0.3; // минимальная скорость при развороте (30%)
    const MIN_MOVEMENT_THRESHOLD: f32 = 0.01; // минимальная velocity для rotation

    let delta_time = time.delta_secs();

    for event in events.read() {
        let Some(actor_node) = visuals.visuals.get(&event.entity).cloned() else {
            continue;
        };

        let mut body = actor_node.cast::<CharacterBody3D>();

        // Проверяем AI state — в бою не поворачиваем (weapon aim system уже поворачивает)
        let Ok(ai_state) = ai_query.get(event.entity) else {
            continue;
        };

        let in_combat = matches!(
            ai_state,
            voidrun_simulation::ai::AIState::Combat { .. }
        );

        // Применяем safe_velocity от NavigationAgent3D
        let safe_vel_godot = Vector3::new(
            event.safe_velocity.x,
            body.get_velocity().y, // Сохраняем Y (гравитация)
            event.safe_velocity.z,
        );

        // Вычисляем целевое направление из safe_velocity (XZ plane)
        let safe_vel_xz = Vec3::new(event.safe_velocity.x, 0.0, event.safe_velocity.z);
        let vel_length = safe_vel_xz.length();

        // Если safe_velocity = 0 но desired_velocity != 0 (цель позади или target reached)
        // → используем desired_velocity для определения направления поворота
        let desired_vel_xz = Vec3::new(event.desired_velocity.x, 0.0, event.desired_velocity.z);
        let desired_length = desired_vel_xz.length();

        let (mut target_direction, should_move) = if vel_length < MIN_MOVEMENT_THRESHOLD {
            if desired_length > MIN_MOVEMENT_THRESHOLD {
                // Safe = 0, но desired != 0 → поворачиваемся к desired direction, не двигаемся
                (desired_vel_xz / desired_length, false)
            } else {
                // Нет ни safe, ни desired → ничего не делаем
                body.set_velocity(safe_vel_godot);
                body.move_and_slide();
                continue;
            }
        } else {
            // Нормальное движение: используем safe_velocity
            (safe_vel_xz / vel_length, true)
        };

        // В бою НЕ поворачиваемся (weapon aim system уже управляет rotation)
        let (new_direction, angle_diff) = if in_combat {
            // В бою: просто применяем velocity, rotation не трогаем
            (target_direction, 0.0) // angle_diff = 0 → no velocity scaling
        } else {
            // Вне боя: плавный поворот к направлению движения
            let godot_basis = body.get_global_basis();
            let forward_godot = -godot_basis.col_c(); // Godot forward = -Z axis
            let current_dir = Vec3::new(forward_godot.x, 0.0, forward_godot.z).normalize();

            // Вычисляем угол между текущим и целевым направлением
            let dot = current_dir.dot(target_direction).clamp(-1.0, 1.0);
            let angle_diff = dot.acos(); // radians [0, PI]

            // Константный поворот: поворачиваемся на фиксированный угол ЗА ФРЕЙМ
            const MAX_ROTATION_PER_FRAME: f32 = 0.2; // радиан за frame (~11.5° за frame при 60fps)

            let new_dir = if angle_diff <= MAX_ROTATION_PER_FRAME {
                // Маленький угол → поворачиваемся сразу к цели
                target_direction
            } else {
                // Большой угол → вычисляем направление после поворота на MAX_ROTATION_PER_FRAME
                // Используем формулу: new = current * cos(angle) + perp * sin(angle)
                // где perp — перпендикулярный вектор в плоскости XZ в сторону target

                // Вычисляем перпендикулярный вектор (в сторону target)
                // cross product с Y-axis даёт перпендикуляр в XZ plane
                let cross = Vec3::new(current_dir.z, 0.0, -current_dir.x); // Перпендикуляр к current

                // Определяем знак поворота (по часовой или против)
                let sign = if cross.dot(target_direction) >= 0.0 { 1.0 } else { -1.0 };

                // Поворот на MAX_ROTATION_PER_FRAME
                let cos_a = MAX_ROTATION_PER_FRAME.cos();
                let sin_a = MAX_ROTATION_PER_FRAME.sin() * sign;

                (current_dir * cos_a + cross * sin_a).normalize()
            };

            (new_dir, angle_diff)
        };

        // Velocity масштабируется косинусом угла (замедление при повороте)
        // НО только если НЕ в бою (в бою двигаемся на полной скорости)
        // cos(0°) = 1.0 (полная скорость), cos(90°) = 0.0 (почти стоп), cos(180°) = -1.0
        let velocity_scale = angle_diff
            .cos()
            .max(0.0) // Clamp negative values (не двигаемся назад)
            .powf(VELOCITY_ANGLE_FACTOR)
            .max(MIN_VELOCITY_SCALE);

        let scaled_velocity = Vector3::new(
            safe_vel_godot.x * velocity_scale,
            safe_vel_godot.y, // Сохраняем Y (гравитация)
            safe_vel_godot.z * velocity_scale,
        );

        // Применяем rotation через look_at (только если НЕ в бою)
        if !in_combat {
            let look_at_pos = Vector3::new(
                body.get_position().x + new_direction.x,
                body.get_position().y,
                body.get_position().z + new_direction.z,
            );
            body.look_at(look_at_pos);
        }

        // Применяем velocity только если should_move = true
        if should_move {
            body.set_velocity(scaled_velocity);
        } else {
            // Не двигаемся, только поворачиваемся
            body.set_velocity(Vector3::new(0.0, safe_vel_godot.y, 0.0)); // Сохраняем Y для гравитации
        }

        body.move_and_slide();
    }
}

fn log_every_30_frames(message: &str) {
    static mut FRAME_COUNTER: u32 = 0;
    unsafe {
        FRAME_COUNTER += 1;
        if FRAME_COUNTER % 30 == 0 {
            voidrun_simulation::log(message);
        }
    }
}
