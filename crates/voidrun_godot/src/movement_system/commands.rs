//! Movement command processing (MovementCommand → NavigationAgent3D).

use crate::shared::VisualRegistry;
use crate::los_helpers::check_line_of_sight;
use bevy::prelude::*;
use godot::classes::{BoxMesh, CharacterBody3D, Material, MeshInstance3D, NavigationAgent3D, StandardMaterial3D};
use godot::prelude::*;
use voidrun_simulation::{MovementCommand, NavigationState};
use voidrun_simulation::logger;

/// Adjust desired distance based on LOS check (stateful iteration).
///
/// Algorithm:
/// 1. If current_distance is None → initialize from max_distance
/// 2. Check LOS at current actor position to target
/// 3. If LOS clear → keep current_distance (не увеличиваем обратно)
/// 4. If LOS blocked → decrease current_distance by 2m
/// 5. If distance < 2m → clamp to 2m (minimum, wait state)
///
/// NavigationAgent will pathfind to closer position, which may clear LOS.
/// Distance iteratively decreases each frame until LOS clears.
///
/// # Parameters
/// - `from_entity`: Shooter/follower entity
/// - `to_entity`: Target entity
/// - `current_distance`: Current adjusted distance (from NavigationState)
/// - `max_distance`: Maximum distance (from weapon range, for initialization)
/// - `visuals`: VisualRegistry for Godot nodes
/// - `scene_root`: SceneRoot for raycast
///
/// # Returns
/// - Adjusted desired_distance for NavigationAgent
pub(super) fn adjust_distance_for_los(
    from_entity: Entity,
    to_entity: Entity,
    current_distance: Option<f32>,
    max_distance: f32,
    visuals: &NonSend<VisualRegistry>,
    scene_root: &NonSend<crate::shared::SceneRoot>,
) -> f32 {
    const MIN_DISTANCE: f32 = 2.0; // Минимальная дистанция (метры)
    const DISTANCE_STEP: f32 = 2.0; // Шаг уменьшения дистанции (метры)

    // Инициализируем current_distance если None
    let current = current_distance.unwrap_or(max_distance);

    // Проверяем LOS
    match check_line_of_sight(from_entity, to_entity, visuals, scene_root) {
        Some(true) => {
            // LOS clear → используем текущую distance (не увеличиваем)
            current
        }
        Some(false) => {
            // LOS blocked → подходим ближе (уменьшаем distance)
            let new_distance = (current - DISTANCE_STEP).max(MIN_DISTANCE);

            // Логируем только если distance изменилась
            if (new_distance - current).abs() > 0.1 {
                logger::log(&format!(
                    "🔄 LOS blocked: {:?} → {:?}, reducing distance {:.1}m → {:.1}m",
                    from_entity, to_entity, current, new_distance
                ));
            }

            new_distance
        }
        None => {
            // Raycast failed → используем current distance (fallback)
            current
        }
    }
}

/// Debug: создаёт красный box marker в указанной позиции
#[allow(dead_code)]
pub(super) fn spawn_debug_marker(position: Vector3, scene_root: &mut Gd<Node>) {
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

                logger::log(&format!(
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
                    nav_state.current_follow_distance = None; // Сброс distance при смене target

                    logger::log(&format!(
                        "Entity {:?}: new FollowEntity target {:?}, reset reached flag + distance",
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

                logger::log(&format!(
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

                logger::log(&format!(
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
