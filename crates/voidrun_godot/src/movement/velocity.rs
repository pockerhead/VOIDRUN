//! Velocity systems (retreat, safe velocity, gravity).
//!
//! Системы:
//! - `apply_retreat_velocity_main_thread`: Применение RetreatFrom movement
//! - `apply_safe_velocity_system`: Применение safe_velocity от NavigationAgent3D avoidance
//! - `apply_gravity_to_all_actors`: Гравитация для всех акторов (каждый frame)

use crate::shared::VisualRegistry;
use bevy::prelude::*;
use godot::classes::CharacterBody3D;
use godot::prelude::*;
use voidrun_simulation::MovementCommand;

/// Применение retreat velocity (движение назад от target)
///
/// ⚠️ TODO (Phase 3 - Procgen): ЗАМЕНИТЬ НА NavigationAgent3D!
///
/// **Проблема:** Текущая реализация использует прямолинейное движение назад.
/// NPCs падают с уровней в пропасть при retreat (нет obstacle avoidance).
///
/// **Решение:** Использовать NavigationAgent3D.set_target_position() для retreat:
/// - Рассчитать безопасную точку ПОЗАДИ NPC (raycast/navmesh query)
/// - NavigationAgent3D найдёт путь с учётом препятствий
/// - Работает так же как FollowEntity, но target = safe retreat point
///
/// **Status:** Временно отключено (return). Включить после procgen когда будут уровни с обрывами.
pub fn apply_retreat_velocity_main_thread(
    _query: Query<(Entity, &MovementCommand)>,
    _visuals: NonSend<VisualRegistry>,
    _transform_events: EventWriter<voidrun_simulation::ai::GodotTransformEvent>,
) {
    return; // TODO: Implement via NavigationAgent3D (Phase 3)

    // OLD IMPLEMENTATION (прямолинейное движение - NPCs падают в пропасть):
    // const RETREAT_SPEED: f32 = 3.0;
    // for (entity, command) in _query.iter() {
    //     let MovementCommand::RetreatFrom { target } = command else { continue; };
    //     let Some(actor_node) = _visuals.visuals.get(&entity).cloned() else { continue; };
    //     let mut body = actor_node.cast::<CharacterBody3D>();
    //     let Some(target_node) = _visuals.visuals.get(target) else { continue; };
    //     let current_pos = body.get_global_position();
    //     let target_pos = target_node.get_global_position();
    //     let retreat_direction = -(target_pos - current_pos).normalized();
    //     let velocity = Vector3::new(
    //         retreat_direction.x * RETREAT_SPEED,
    //         body.get_velocity().y,
    //         retreat_direction.z * RETREAT_SPEED,
    //     );
    //     body.look_at(Vector3::new(target_pos.x, body.get_position().y, target_pos.z));
    //     body.set_velocity(velocity);
    //     body.move_and_slide();
    //     _transform_events.write(voidrun_simulation::ai::GodotTransformEvent::PositionChanged {
    //         entity,
    //         position: Vec3::new(body.get_position().x, body.get_position().y, body.get_position().z),
    //     });
    // }
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
/// - В бою НЕ поворачиваемся (weapon aim system уже управляет rotation)
pub fn apply_safe_velocity_system(
    mut events: EventReader<crate::navigation::SafeVelocityComputed>,
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
        // NOTE: velocity.y управляется отдельной системой apply_gravity_to_all_actors
        if should_move {
            body.set_velocity(scaled_velocity);
        } else {
            // Не двигаемся, только поворачиваемся
            // Сохраняем текущую Y velocity (gravity уже применена в apply_gravity_to_all_actors)
            let current_y = body.get_velocity().y;
            body.set_velocity(Vector3::new(0.0, current_y, 0.0));
        }

        body.move_and_slide();
    }
}

/// Применение гравитации ко ВСЕМ акторам (каждый frame, как в 3d-rpg)
///
/// Flow:
/// 1. Для ВСЕХ акторов (With<Actor>) каждый frame
/// 2. Проверяем is_on_floor() (встроенное в CharacterBody3D)
/// 3. На земле: velocity.y = 0 (или JUMP_SPEED если JumpIntent)
/// 4. В воздухе: velocity.y -= GRAVITY * delta
/// 5. Вызываем move_and_slide() для обновления collision detection
///
/// КРИТИЧНО:
/// - Запускается ПЕРЕД apply_navigation_velocity (первая в цепочке)
/// - Работает для Idle/Moving/Combat акторов (независимо от movement state)
/// - move_and_slide() вызывается КАЖДЫЙ FRAME для КАЖДОГО актора
///
/// Архитектура как в 3d-rpg:
/// - Manual gravity calculation (не Physics3D engine)
/// - CharacterBody3D для deterministic movement
/// - is_on_floor() для grounding detection
pub fn apply_gravity_to_all_actors(
    actor_query: Query<Entity, With<voidrun_simulation::Actor>>,
    mut jump_events: EventReader<voidrun_simulation::JumpIntent>,
    visuals: NonSend<VisualRegistry>,
    time: Res<Time>,
) {
    use godot::classes::CharacterBody3D;
    use std::collections::HashSet;

    // Параметры гравитации (как в 3d-rpg)
    const GRAVITY: f32 = 9.8; // m/s² (Earth gravity)
    const JUMP_SPEED: f32 = 4.5; // m/s (vertical velocity)

    let delta = time.delta_secs();

    // Собираем entities из JumpIntent events
    let jump_entities: HashSet<Entity> = jump_events.read().map(|e| e.entity).collect();

    for entity in actor_query.iter() {
        let Some(actor_node) = visuals.visuals.get(&entity).cloned() else {
            continue;
        };

        let mut body = actor_node.cast::<CharacterBody3D>();

        // Читаем текущую velocity
        let mut velocity = body.get_velocity();

        // Manual gravity (как в 3d-rpg: player.gd:68-71, enemy.gd:41-45)
        if body.is_on_floor() {
            // На земле → проверяем JumpIntent
            if jump_entities.contains(&entity) {
                velocity.y = JUMP_SPEED; // Прыгаем!
                voidrun_simulation::logger::log(&format!(
                    "Entity {:?}: jump! velocity.y = {:.1} m/s",
                    entity, JUMP_SPEED
                ));
            } else {
                velocity.y = 0.0; // Стоим на земле
            }
        } else {
            // В воздухе → применяем гравитацию
            velocity.y -= GRAVITY * delta;
        }

        // Применяем обновлённую velocity
        body.set_velocity(velocity);

        // ✅ КРИТИЧНО: move_and_slide() каждый frame для collision detection
        // Без этого CharacterBody3D не обновляет is_on_floor() и проваливается сквозь пол
        body.move_and_slide();
    }
}
