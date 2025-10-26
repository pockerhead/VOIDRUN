//! Line-of-Sight (LOS) helpers — raycast utilities для проверки видимости
//!
//! Общие утилиты для LOS проверок используемые в:
//! - ai_combat_decision.rs (melee/ranged attack decisions)
//! - movement_system.rs (NavigationAgent distance adjustment)
//! - weapon_system.rs (fire intent validation)

use bevy::prelude::*;
use godot::prelude::*;
use voidrun_simulation::logger;

use crate::shared::VisualRegistry;

/// Check line-of-sight between two entities using Godot physics raycast.
///
/// Returns:
/// - `Some(true)` if LOS is clear (no obstacles between entities)
/// - `Some(false)` if LOS is blocked (obstacle/wall between entities)
/// - `None` if raycast failed (missing nodes, invalid world state)
///
/// # Implementation
///
/// Uses `PhysicsDirectSpaceState3D::intersect_ray()` to check for collisions:
/// - Raycast from `from_entity` global position to `to_entity` global position
/// - Collision mask: Layer 2 (actors) + Layer 3 (environment/obstacles)
/// - If hit collider == `to_entity` → LOS clear (direct hit)
/// - If hit collider != `to_entity` → LOS blocked (obstacle in path)
/// - If no hit → LOS clear (no obstacles)
///
/// # YAGNI Note
///
/// Пока проверяем только binary LOS (clear/blocked).
/// Будущие улучшения (если понадобится):
/// - Partial cover detection (ray width, multiple rays)
/// - Dynamic obstacles (moving entities)
pub fn check_line_of_sight(
    from_entity: Entity,
    to_entity: Entity,
    visuals: &NonSend<VisualRegistry>,
    scene_root: &NonSend<crate::shared::SceneRoot>,
) -> Option<bool> {
    // 1. Get Godot nodes for both entities
    let Some(from_node_3d) = visuals.visuals.get(&from_entity) else {
        return None;
    };
    let Some(to_node_3d) = visuals.visuals.get(&to_entity) else {
        return None;
    };

    // Cast to CharacterBody3D
    let Ok(from_node) = from_node_3d.clone().try_cast::<godot::classes::CharacterBody3D>() else {
        return None;
    };
    let Ok(to_node) = to_node_3d.clone().try_cast::<godot::classes::CharacterBody3D>() else {
        return None;
    };

    // 2. Get positions (eye-level: Y+0.8 для головы)
    let from_pos = from_node.get_global_position() + Vector3::new(0.0, 0.8, 0.0);
    let to_pos = to_node.get_global_position() + Vector3::new(0.0, 0.8, 0.0);

    // 3. Raycast через PhysicsDirectSpaceState3D
    let world = scene_root.node.get_world_3d();
    let Some(mut world) = world else {
        logger::log_error("check_line_of_sight: World3D не найден");
        return None;
    };

    let space = world.get_direct_space_state();
    let Some(mut space) = space else {
        logger::log_error("check_line_of_sight: PhysicsDirectSpaceState3D не найден");
        return None;
    };

    // Создаём raycast query
    let query = godot::classes::PhysicsRayQueryParameters3D::create(from_pos, to_pos);
    let Some(mut query) = query else {
        logger::log_error("check_line_of_sight: PhysicsRayQueryParameters3D::create failed");
        return None;
    };

    // Collision mask: Actors + Environment (LOS check)
    query.set_collision_mask(crate::collision_layers::COLLISION_MASK_RAYCAST_LOS);

    let empty_array = godot::prelude::Array::new();
    query.set_exclude(&empty_array); // Не исключаем ничего (проверяем все коллизии)

    // Выполняем raycast
    let result = space.intersect_ray(&query);

    // 4. Анализируем результат
    if result.is_empty() {
        // Нет коллизий → LOS clear (прямая видимость)
        return Some(true);
    }

    // Есть коллизия → проверяем что это target entity
    let Some(collider) = result.get("collider") else {
        logger::log_error("check_line_of_sight: raycast result missing 'collider'");
        return None;
    };

    // Получаем Variant → Gd<Node>
    let Ok(collider_node) = collider.try_to::<Gd<godot::classes::Node>>() else {
        logger::log_error("check_line_of_sight: collider не является Node");
        return None;
    };

    let collider_id = collider_node.instance_id();

    // Проверяем что collider == to_entity node
    if collider_id == to_node.instance_id() {
        // Попали точно в target → LOS clear
        Some(true)
    } else {
        // Попали в препятствие → LOS blocked
        Some(false)
    }
}
