//! Visual sync systems — ECS Changed<T> → Godot visual updates
//!
//! Architecture: ADR-004 (NonSend resources + Changed<T> queries)
//! All systems — main thread only (NonSendMut<VisualRegistry>)
//!
//! NAMING CONVENTION: `_main_thread` суффикс = Godot API calls

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{
    MeshInstance3D, Label3D, Node, PackedScene, ResourceLoader,
    StandardMaterial3D, Material, NavigationAgent3D,
    base_material_3d::BillboardMode,
};
use voidrun_simulation::{Actor, Health, Stamina};
use voidrun_simulation::ai::AIState;
use crate::systems::visual_registry::VisualRegistry;

/// Spawn visuals for newly created actors
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
/// ADR-005: Spawn на StrategicPosition + PostSpawn коррекция
pub fn spawn_actor_visuals_main_thread(
    query: Query<(Entity, &Actor, &Health, &Stamina, &voidrun_simulation::StrategicPosition, &voidrun_simulation::PrefabPath), Added<Actor>>,
    mut visuals: NonSendMut<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
    mut transform_events: EventWriter<voidrun_simulation::ai::GodotTransformEvent>,
) {
    for (entity, actor, health, stamina, strategic_pos, prefab_path) in query.iter() {
        // Загружаем TSCN prefab из PrefabPath компонента
        let mut loader = ResourceLoader::singleton();
        let scene = loader.load_ex(&prefab_path.path).done();

        let Some(scene) = scene else {
            voidrun_simulation::log(&format!("❌ Failed to load prefab: {}", prefab_path.path));
            continue;
        };

        let packed_scene: Gd<PackedScene> = scene.cast();

        let Some(instance) = packed_scene.instantiate() else {
            voidrun_simulation::log(&format!("❌ Failed to instantiate prefab: {}", prefab_path.path));
            continue;
        };

        let mut actor_node = instance.cast::<Node3D>();

        // Спавним на стратегической позиции (StrategicPosition → world coordinates)
        let spawn_pos = strategic_pos.to_world_position(0.5); // Y=0.5 (над землёй)
        actor_node.set_position(Vector3::new(spawn_pos.x, spawn_pos.y, spawn_pos.z));

        // Цвет фракции — красим все MeshInstance3D дочерние ноды
        let faction_color = match actor.faction_id {
            1 => Color::from_rgb(0.2, 0.6, 1.0), // Blue
            2 => Color::from_rgb(0.8, 0.2, 0.2), // Red
            3 => Color::from_rgb(0.2, 0.8, 0.2), // Green
            _ => Color::from_rgb(0.5, 0.5, 0.5), // Gray
        };

        // Красим все mesh instances в prefab
        for i in 0..actor_node.get_child_count() {
            if let Some(mut child) = actor_node.get_child(i).and_then(|c| c.try_cast::<MeshInstance3D>().ok()) {
                let mut material = StandardMaterial3D::new_gd();
                material.set_albedo(faction_color);
                child.set_surface_override_material(0, &material.upcast::<Material>());
            }
        }

        // AI state label (над головой, выше health)
        let mut ai_label = Label3D::new_alloc();
        let ai_text = format!("AI");
        ai_label.set_text(ai_text.as_str());
        ai_label.set_pixel_size(0.004);
        ai_label.set_billboard_mode(BillboardMode::ENABLED);
        ai_label.set_position(Vector3::new(0.0, 1.4, 0.0));
        ai_label.set_modulate(Color::from_rgb(0.8, 0.8, 0.2)); // Желтый
        actor_node.add_child(&ai_label.clone().upcast::<Node>());

        // Health label над головой
        let mut health_label = Label3D::new_alloc();
        let health_text = format!("HP: {}/{}", health.current, health.max);
        health_label.set_text(health_text.as_str());
        health_label.set_pixel_size(0.005);
        health_label.set_billboard_mode(BillboardMode::ENABLED);
        health_label.set_position(Vector3::new(0.0, 1.2, 0.0));
        actor_node.add_child(&health_label.clone().upcast::<Node>());

        // Stamina label под health
        let mut stamina_label = Label3D::new_alloc();
        let stamina_text = format!("Stamina: {:.0}/{:.0}", stamina.current, stamina.max);
        stamina_label.set_text(stamina_text.as_str());
        stamina_label.set_pixel_size(0.004);
        stamina_label.set_billboard_mode(BillboardMode::ENABLED);
        stamina_label.set_position(Vector3::new(0.0, 1.0, 0.0));
        stamina_label.set_modulate(Color::from_rgb(0.2, 0.8, 0.2)); // Зелёный
        actor_node.add_child(&stamina_label.clone().upcast::<Node>());

        // Добавляем в сцену через SceneRoot (СНАЧАЛА добавляем в дерево!)
        let mut root = scene_root.node.clone();
        root.add_child(&actor_node.clone().upcast::<Node>());

        // КРИТИЧНО: Устанавливаем collision layers явно (даже если есть в TSCN)
        // Actors (layer 2) коллидируют с Actors + Environment (layers 2,3)
        if let Ok(mut char_body) = actor_node.clone().try_cast::<godot::classes::CharacterBody3D>() {
            char_body.set_collision_layer(crate::collision_layers::COLLISION_LAYER_ACTORS);
            char_body.set_collision_mask(crate::collision_layers::COLLISION_MASK_ACTORS);
            voidrun_simulation::log("  → Collision layers set: Actors (layer 2, mask 2|4)");
        }

        // ПОСЛЕ добавления в SceneTree — добавляем NavigationAgent3D как прямой ребёнок actor_node
        // (NavigationAgent требует чтобы parent был уже в дереве сцены)
        let mut nav_agent = NavigationAgent3D::new_alloc();
        nav_agent.set_name("NavigationAgent3D"); // ВАЖНО: задаём имя явно!

        nav_agent.set_debug_enabled(true);
        nav_agent.set_debug_path_custom_color(Color::from_rgb(0.8, 0.2, 0.2));

        // КРИТИЧНО: avoidance = true (используем velocity_computed signal для obstacle avoidance)
        nav_agent.set_avoidance_enabled(true);
        nav_agent.set_radius(0.5); // Actor collision radius (как CapsuleShape3D)
        nav_agent.set_max_speed(10.0); // MOVE_SPEED = 10.0 (используется NavigationServer для avoidance)

        actor_node.add_child(&nav_agent.upcast::<Node>());
        voidrun_simulation::log("  → NavigationAgent3D added (avoidance enabled)");

        // Создаём AvoidanceReceiver для обработки velocity_computed signal
        let mut avoidance_receiver = Gd::<crate::avoidance_receiver::AvoidanceReceiver>::from_init_fn(|base| {
            crate::avoidance_receiver::AvoidanceReceiver::init(base)
        });
        avoidance_receiver.set_name("AvoidanceReceiver");

        // Устанавливаем entity_id (для callback → ECS event)
        avoidance_receiver.bind_mut().entity_id = entity.to_bits() as i64;

        // Устанавливаем путь к SimulationBridge (для EventWriter доступа)
        let bridge_path = root.get_path();
        avoidance_receiver.bind_mut().simulation_bridge_path = bridge_path;

        // Добавляем как child actor_node
        actor_node.add_child(&avoidance_receiver.upcast::<Node>());
        voidrun_simulation::log("  → AvoidanceReceiver added (velocity_computed signal)");

        // Регистрируем в VisualRegistry (Entity → Godot Node + reverse mapping)
        let instance_id = actor_node.instance_id();
        visuals.visuals.insert(entity, actor_node.clone());
        visuals.node_to_entity.insert(instance_id, entity);
        visuals.health_labels.insert(entity, health_label);
        visuals.stamina_labels.insert(entity, stamina_label);
        visuals.ai_state_labels.insert(entity, ai_label);

        // КРИТИЧНО: actor_node теперь САМ CharacterBody3D — регистрируем его для projectile collision
        let actor_id = actor_node.instance_id();
        crate::projectile::register_collision_body(actor_id, entity);
        voidrun_simulation::log(&format!("  → CharacterBody3D (root) mapped for entity {:?}", entity));

        // КРИТИЧНО: PostSpawn коррекция — отправляем точную позицию обратно в ECS
        let final_position = actor_node.get_global_position();
        transform_events.write(voidrun_simulation::ai::GodotTransformEvent::PostSpawn {
            entity,
            position: Vec3::new(final_position.x, final_position.y, final_position.z),
        });

        voidrun_simulation::log(&format!("✅ Spawned visual (prefab: {}) at strategic {:?}", prefab_path.path, strategic_pos));
    }
}

/// Sync health changes → Godot Label3D
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn sync_health_labels_main_thread(
    query: Query<(Entity, &Health), Changed<Health>>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    for (entity, health) in query.iter() {
        let Some(label) = visuals.health_labels.get_mut(&entity) else {
            continue;
        };

        let text = format!("HP: {}/{}", health.current, health.max);
        label.set_text(text.as_str());
    }
}

/// Sync stamina changes → Godot Label3D
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn sync_stamina_labels_main_thread(
    query: Query<(Entity, &Stamina), Changed<Stamina>>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    for (entity, stamina) in query.iter() {
        let Some(label) = visuals.stamina_labels.get_mut(&entity) else {
            continue;
        };

        let text = format!("Stamina: {:.0}/{:.0}", stamina.current, stamina.max);
        label.set_text(text.as_str());
    }
}

/// Sync AI state changes → Godot Label3D
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn sync_ai_state_labels_main_thread(
    query: Query<(Entity, &AIState), Changed<AIState>>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    for (entity, state) in query.iter() {
        let Some(label) = visuals.ai_state_labels.get_mut(&entity) else {
            continue;
        };

        let text = format!("[{:?}]", state);
        label.set_text(text.as_str());
    }
}

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
    use godot::classes::CharacterBody3D;

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
            // 1. ОТКЛЮЧАЕМ COLLISION (layer/mask = 0)
            // ========================================
            body.set_collision_layer(0);
            body.set_collision_mask(0);

            // ========================================
            // 2. ОТКЛЮЧАЕМ NAVIGATIONAGENT3D
            // ========================================
            if let Some(mut nav_agent) = actor_node.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D") {
                nav_agent.set_avoidance_enabled(false); // Отключить avoidance (не мешать другим)
                nav_agent.set_velocity_forced(Vector3::ZERO); // Остановить движение
                nav_agent.set_target_position(actor_node.get_global_position()); // Сбросить target (stop pathfinding)
                voidrun_simulation::log(&format!("  → NavigationAgent3D disabled (entity {:?})", entity));
            }

            // ========================================
            // 3. УДАЛЯЕМ VISIONCONE (если есть)
            // ========================================
            if let Some(mut vision_cone) = actor_node.try_get_node_as::<godot::classes::Area3D>("VisionCone") {
                vision_cone.set_monitoring(false); // Отключить collision detection
                vision_cone.queue_free(); // Удалить node (отложенно)
                voidrun_simulation::log(&format!("  → VisionCone removed (entity {:?})", entity));
            }

            // ========================================
            // 4. ОТКЛЮЧАЕМ AVOIDANCERECEIVER (signal callbacks)
            // ========================================
            if let Some(mut receiver) = actor_node.try_get_node_as::<Node>("AvoidanceReceiver") {
                receiver.set_process_mode(godot::classes::node::ProcessMode::DISABLED);
                voidrun_simulation::log(&format!("  → AvoidanceReceiver disabled (entity {:?})", entity));
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

            voidrun_simulation::log(&format!(
                "💀 Entity {:?} died — FULL CLEANUP: collision off, nav off, vision off, gray painted, despawn in 5 sec",
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
            voidrun_simulation::log(&format!("🗑️ Removing Godot node for entity {:?}", entity));
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
