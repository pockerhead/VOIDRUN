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
use voidrun_simulation::MovementCommand;

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
/// КРИТИЧНО: set_target_position() вызывается КАЖДЫЙ ФРЕЙМ (не только Changed<>)
/// NavigationAgent3D требует постоянного обновления target для корректного pathfinding.
pub fn process_movement_commands_main_thread(
    query: Query<(Entity, &MovementCommand), Changed<MovementCommand>>, // Убрали Changed<MovementCommand>
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, command) in query.iter() {
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
                nav_agent.set_target_position(actor_node.get_position());
            }
            MovementCommand::MoveToPosition { target } => {
                let target_vec = Vector3::new(target.x, target.y, target.z);
                nav_agent.set_target_position(target_vec);
            }
            _ => {}
        }
    }
}

/// Применение NavigationAgent3D → CharacterBody3D движение
///
/// Берём get_next_path_position() от NavigationAgent и применяем velocity.
/// Avoidance отключён — простой pathfinding для single-player game.
pub fn apply_navigation_velocity_main_thread(
    query: Query<(Entity, &voidrun_simulation::ai::AIState), With<voidrun_simulation::Actor>>,
    visuals: NonSend<VisualRegistry>,
) {
    const MOVE_SPEED: f32 = 5.0; // метры в секунду

    for (entity, state) in query.iter() {
        if let voidrun_simulation::ai::AIState::Combat { target } = state {
            continue;
        }

        // actor_node теперь САМ CharacterBody3D (root node из TSCN)
        let Some(actor_node) = visuals.visuals.get(&entity).cloned() else {
            continue;
        };

        // Cast root node к CharacterBody3D
        let mut body = actor_node.cast::<CharacterBody3D>();

        let Some(mut nav_agent) =
            body.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D")
        else {
            continue;
        };

        // КРИТИЧНО: Проверяем что путь валиден (NavigationAgent имеет цель и рассчитал путь)
        // is_target_reachable() = false если путь не найден или цель не установлена
        if !nav_agent.is_target_reachable() {
            // Нет валидного пути — стоим на месте
            nav_agent.set_velocity(Vector3::ZERO);
            body.set_velocity(Vector3::ZERO);
            continue;
        }

        // Проверяем достигли ли цели (как enemy.gd:36)
        if nav_agent.is_target_reached() {
            log_every_10_frames(&format!("[Movement] target reached"));
            nav_agent.set_velocity(Vector3::ZERO);
            body.set_velocity(Vector3::ZERO);
            continue;
        }

        // Вычисляем направление к следующей waypoint (enemy.gd:73-76)
        let next_pos = nav_agent.get_next_path_position();
        let current_pos = body.get_global_position();
        let target_pos = nav_agent.get_target_position();

        // Диагностика: логируем target, reachable, next waypoint
        log_every_10_frames(&format!(
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

        // Вычисляем velocity в м/с (как enemy.gd line 37)
        let velocity = Vector3::new(
            local_direction.x * MOVE_SPEED,
            body.get_velocity().y,  // Сохраняем Y (гравитация)
            local_direction.z * MOVE_SPEED,
        );
        let look_at_pos = Vector3::new(next_pos.x, body.get_position().y, next_pos.z);
        // Поворачиваем актора в направлении движения (enemy.gd line 71)
        body.look_at(look_at_pos);

        // Применяем velocity и двигаем (enemy.gd line 84-85)
        body.set_velocity(velocity);
        body.move_and_slide();
    }
}

fn log_every_10_frames(message: &str) {
    static mut FRAME_COUNTER: u32 = 0;
    unsafe {
        FRAME_COUNTER += 1;
        if FRAME_COUNTER % 10 == 0 {
            voidrun_simulation::log(message);
        }
    }
}
