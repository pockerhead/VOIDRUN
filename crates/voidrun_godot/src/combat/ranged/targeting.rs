//! Target selection and weapon aim systems.

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::Node3D;
use voidrun_simulation::*;
use crate::shared::VisualRegistry;
use voidrun_simulation::logger;
// ============================================================================
// Systems: Target Switching + Aim
// ============================================================================

/// System: Dynamic target switching (SlowUpdate schedule, 0.3 Hz)
///
/// Для ВСЕХ акторов в AIState::Combat:
/// - Проверяет ближайшего ВИДИМОГО врага из SpottedEnemies (VisionCone + LOS raycast)
/// - Если ближайший враг ≠ текущий target → переключает target
///
/// **Результат:** AI всегда атакует ближайшего видимого врага (dynamic target prioritization)
///
/// **Schedule:** SlowUpdate (0.3 Hz = ~3 раза в секунду)
/// - Экономия CPU (не нужно каждый frame)
/// - Более реалистичное поведение AI (время реакции ~0.3с)
/// - Избегаем "perfect play" эффект (instant target switching)
///
/// ВАЖНО: НЕ зависит от WeaponFireIntent events (отдельная система)
pub fn update_combat_targets_main_thread(
    mut actors: Query<(Entity, &Actor, &mut ai::AIState, &ai::SpottedEnemies), With<Actor>>,
    all_actors: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::shared::SceneRoot>,
) {

    // Получаем PhysicsDirectSpaceState3D один раз для всех акторов
    let world = scene_root.node.get_world_3d();
    let Some(mut world) = world else {
        return;
    };

    let space = world.get_direct_space_state();
    let Some(mut space) = space else {
        return;
    };

    for (entity, actor, mut ai_state, spotted_enemies) in actors.iter_mut() {
        // Обрабатываем только Combat state
        let ai::AIState::Combat { target: current_target } = ai_state.as_ref() else {
            continue;
        };

        // Получаем shooter node для distance calculation
        let Some(shooter_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let shooter_pos = shooter_node.get_global_position();
        let shooter_eye = shooter_pos + Vector3::new(0.0, 0.8, 0.0); // Eye level

        // Ищем БЛИЖАЙШЕГО ВИДИМОГО врага из SpottedEnemies
        let mut closest_visible_enemy: Option<(Entity, f32)> = None;

        for &enemy_entity in &spotted_enemies.enemies {
            // Проверяем что враг жив (есть в actors)
            let Ok(enemy_actor) = all_actors.get(enemy_entity) else {
                continue;
            };

            // Проверяем faction (только враги)
            if enemy_actor.faction_id == actor.faction_id {
                continue;
            }

            // Получаем Godot node для distance + LOS check
            let Some(enemy_node) = visuals.visuals.get(&enemy_entity) else {
                continue;
            };

            let enemy_pos = enemy_node.get_global_position();
            let distance_to_enemy = (enemy_pos - shooter_pos).length();

            // ✅ LOS CHECK: raycast от shooter к enemy (eye-level)
            let enemy_eye = enemy_pos + Vector3::new(0.0, 0.8, 0.0);

            let query_params = godot::classes::PhysicsRayQueryParameters3D::create(shooter_eye, enemy_eye);
            let Some(mut query) = query_params else {
                continue;
            };

            query.set_collision_mask(crate::shared::collision::COLLISION_MASK_RAYCAST_LOS);
            let empty_array = godot::prelude::Array::new();
            query.set_exclude(&empty_array);

            let result = space.intersect_ray(&query);

            // Проверяем результат raycast
            if result.is_empty() {
                // Нет коллизий → странно, skip
                continue;
            }

            let Some(collider_variant) = result.get("collider") else {
                continue;
            };

            let Ok(collider_node) = collider_variant.try_to::<Gd<godot::classes::Node>>() else {
                continue;
            };

            let collider_id = collider_node.instance_id();
            let enemy_instance_id = enemy_node.instance_id();

            // Если попали НЕ в enemy → LOS заблокирован, skip
            if collider_id != enemy_instance_id {
                continue;
            }

            // ✅ ВРАГ ВИДИМ! Обновляем ближайшего
            if let Some((_, current_min_dist)) = closest_visible_enemy {
                if distance_to_enemy < current_min_dist {
                    closest_visible_enemy = Some((enemy_entity, distance_to_enemy));
                }
            } else {
                closest_visible_enemy = Some((enemy_entity, distance_to_enemy));
            }
        }

        // Если нашли ближайшего видимого и он НЕ равен текущему target → переключаем
        if let Some((closest_entity, closest_distance)) = closest_visible_enemy {
            if closest_entity != *current_target {
                // ✅ ЗАМЕНЯЕМ TARGET в AIState::Combat
                if let ai::AIState::Combat { ref mut target } = ai_state.as_mut() {
                    let old_target = *target;
                    *target = closest_entity;

                    logger::log(&format!(
                        "🎯 TARGET SWITCH (closest visible): {:?} switches from {:?} to {:?} at {:.1}m",
                        entity, old_target, closest_entity, closest_distance
                    ));
                }
            }
        }
    }

}

/// System: Aim weapon at target (RightHand rotation)
/// Если актёр в Combat state → поворачиваем руку к target
///
/// ВАЖНО: Использует Godot Transform из VisualRegistry (не ECS Transform!)
pub fn weapon_aim_main_thread(
    actors: Query<(Entity, &ai::AIState), With<Actor>>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, state) in actors.iter() {
        // Целимся только в Combat state
        if let ai::AIState::Combat { target } = state {
            // Получаем actor node (shooter)
            let Some(mut actor_node) = visuals.visuals.get(&entity).cloned() else {
                continue;
            };

            // Получаем target node (НЕ ECS Transform — Godot Transform!)
            let Some(target_node) = visuals.visuals.get(target).cloned() else {
                continue;
            };

            // Godot positions (tactical layer — authoritative для aim)
            let target_pos = target_node.get_global_position();
            let actor_pos = actor_node.get_global_position();
            let to_target = target_pos - actor_pos;

            if to_target.length() > 0.01 {
                // Поворачиваем весь actor body к target
                actor_node.look_at(target_pos);

                // Дополнительно поворачиваем RightHand (оружие) к target для точного прицеливания
                if let Some(mut right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") {
                    right_hand.look_at(target_pos);
                }
            }
        }
    }
}
