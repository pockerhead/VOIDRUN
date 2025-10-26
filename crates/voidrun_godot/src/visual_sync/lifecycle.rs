//! Actor lifecycle systems (death handling, despawn)

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{MeshInstance3D, StandardMaterial3D, Material, NavigationAgent3D};
use voidrun_simulation::Health;
use crate::shared::VisualRegistry;
use voidrun_simulation::logger;
/// Disable collision for dead actors (HP == 0) + full cleanup + schedule despawn after 5 sec
///
/// **Complete cleanup for dead actors:**
/// - Отключает collision (layer/mask = 0) у CharacterBody3D
/// - Отключает NavigationAgent3D (avoidance_enabled = false, set_velocity_forced = 0)
/// - Красит все MeshInstance3D в серый цвет
/// - Удаляет VisionCone (Area3D) если есть
/// - Отключает AvoidanceReceiver (для предотвращения signal callbacks)
/// - Добавляет DespawnAfter компонент (desp spawn через 5 сек)
///
/// **Result:** Dead actor больше не мешает живым (no collision, no pathfinding, no vision)
pub fn disable_collision_on_death_main_thread(
    query: Query<(Entity, &Health), Changed<Health>>,
    visuals: NonSend<VisualRegistry>,
    mut commands: Commands,
    time: Res<Time>,
) {
    use godot::classes::{CharacterBody3D, Node};

    for (entity, health) in query.iter() {
        // Проверяем что актёр мёртв (HP == 0)
        if health.current > 0 {
            continue;
        }

        // Получаем Godot node
        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Пробуем получить CharacterBody3D (root node в test_actor.tscn)
        if let Some(mut body) = actor_node.clone().try_cast::<CharacterBody3D>().ok() {
            // ========================================
            // 1. CORPSE COLLISION (только с Environment, не с Actors/Projectiles)
            // ========================================
            // Труп лежит на земле (не проваливается), но не блокирует живых
            body.set_collision_layer(crate::collision_layers::COLLISION_LAYER_CORPSES);
            body.set_collision_mask(crate::collision_layers::COLLISION_MASK_CORPSES);

            // ========================================
            // 2. ОТКЛЮЧАЕМ NAVIGATIONAGENT3D
            // ========================================
            if let Some(mut nav_agent) = actor_node.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D") {
                nav_agent.set_avoidance_enabled(false); // Отключить avoidance (не мешать другим)
                nav_agent.set_velocity_forced(Vector3::ZERO); // Остановить движение
                nav_agent.set_target_position(actor_node.get_global_position()); // Сбросить target (stop pathfinding)
                logger::log(&format!("  → NavigationAgent3D disabled (entity {:?})", entity));
            }

            // ========================================
            // 3. УДАЛЯЕМ VISIONCONE (если есть)
            // ========================================
            if let Some(mut vision_cone) = actor_node.try_get_node_as::<godot::classes::Area3D>("VisionCone") {
                vision_cone.set_monitoring(false); // Отключить collision detection
                vision_cone.queue_free(); // Удалить node (отложенно)
                logger::log(&format!("  → VisionCone removed (entity {:?})", entity));
            }

            // ========================================
            // 4. ОТКЛЮЧАЕМ AVOIDANCERECEIVER (signal callbacks)
            // ========================================
            if let Some(mut receiver) = actor_node.try_get_node_as::<Node>("AvoidanceReceiver") {
                receiver.set_process_mode(godot::classes::node::ProcessMode::DISABLED);
                logger::log(&format!("  → AvoidanceReceiver disabled (entity {:?})", entity));
            }

            // ========================================
            // 5. КРАСИМ ВСЕ MESHINSTANCE3D В СЕРЫЙ
            // ========================================
            for i in 0..body.get_child_count() {
                if let Some(mut mesh) = body.get_child(i).and_then(|c| c.try_cast::<MeshInstance3D>().ok()) {
                    let mut material = StandardMaterial3D::new_gd();
                    material.set_albedo(Color::from_rgb(0.4, 0.4, 0.4)); // Серый
                    mesh.set_surface_override_material(0, &material.upcast::<Material>());
                }
            }

            logger::log(&format!(
                "💀 Entity {:?} died — FULL CLEANUP: corpse collision (Environment only), nav off, vision off, gray painted, despawn in 5 sec",
                entity
            ));

            // ========================================
            // 6. SCHEDULE DESPAWN AFTER 5 SECONDS
            // ========================================
            let despawn_time = time.elapsed_secs() + 5.0;
            commands.entity(entity).insert(voidrun_simulation::combat::DespawnAfter { despawn_time });
        }
    }
}

/// Despawn Godot visuals for despawned ECS entities
///
/// Удаляет Godot nodes когда ECS entity деспавнится.
/// Вызывается в Update после despawn_after_timeout.
pub fn despawn_actor_visuals_main_thread(
    mut removed: RemovedComponents<voidrun_simulation::Actor>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    for entity in removed.read() {
        // Удаляем Godot node
        if let Some(mut node) = visuals.visuals.remove(&entity) {
            logger::log(&format!("🗑️ Removing Godot node for entity {:?}", entity));
            node.queue_free(); // Отложенное удаление (Godot safe)
        }

        // Очищаем все связанные entries в registry
        visuals.health_labels.remove(&entity);
        visuals.stamina_labels.remove(&entity);
        visuals.ai_state_labels.remove(&entity);
        // node_to_entity будет очищен автоматически при queue_free
    }
}

// УДАЛЕНО: sync_transforms_main_thread
// ADR-005: Godot Transform authoritative (не синхронизируем из ECS)
// Transform обновляется через CharacterBody3D.move_and_slide()
// StrategicPosition sync только при zone transitions (0.1-1 Hz)
