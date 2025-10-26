//! Actor visual spawning system

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{
    MeshInstance3D, Label3D, Node, PackedScene, ResourceLoader,
    StandardMaterial3D, Material, NavigationAgent3D,
    base_material_3d::BillboardMode,
};
use voidrun_simulation::{Actor, Health, Stamina};
use crate::shared::VisualRegistry;
use voidrun_simulation::logger;
/// Spawn visuals for newly created actors
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
/// ADR-005: Spawn на StrategicPosition + PostSpawn коррекция
pub fn spawn_actor_visuals_main_thread(
    query: Query<(Entity, &Actor, &Health, &Stamina, Option<&voidrun_simulation::components::EnergyShield>, &voidrun_simulation::StrategicPosition, &voidrun_simulation::PrefabPath), Added<Actor>>,
    mut visuals: NonSendMut<VisualRegistry>,
    scene_root: NonSend<crate::shared::SceneRoot>,
    mut transform_events: EventWriter<voidrun_simulation::ai::GodotTransformEvent>,
) {
    for (entity, actor, health, stamina, shield_opt, strategic_pos, prefab_path) in query.iter() {
        // Загружаем TSCN prefab из PrefabPath компонента
        let mut loader = ResourceLoader::singleton();
        let scene = loader.load_ex(&prefab_path.path).done();

        let Some(scene) = scene else {
            logger::log(&format!("❌ Failed to load prefab: {}", prefab_path.path));
            continue;
        };

        let packed_scene: Gd<PackedScene> = scene.cast();

        let Some(instance) = packed_scene.instantiate() else {
            logger::log(&format!("❌ Failed to instantiate prefab: {}", prefab_path.path));
            continue;
        };

        // ВАЖНО: test_player.tscn имеет root wrapper Node3D с child "Actor" (CharacterBody3D)
        // test_actor.tscn имеет root CharacterBody3D (Actor)
        // Определяем что instantiated: если CharacterBody3D → используем напрямую, иначе используем wrapper
        let (mut scene_node, mut actor_node) = if let Ok(body) = instance.clone().try_cast::<godot::classes::CharacterBody3D>() {
            // Root это CharacterBody3D (test_actor.tscn) → scene_node = actor_node
            let actor = body.upcast::<Node3D>();
            (actor.clone(), actor)
        } else {
            // Root это Node3D wrapper (test_player.tscn) → scene_node = wrapper, actor_node = child "Actor"
            let wrapper = instance.cast::<Node3D>();
            let Some(actor_child) = wrapper.try_get_node_as::<Node3D>("Actor") else {
                logger::log(&format!("❌ Actor child not found in prefab: {}", prefab_path.path));
                continue;
            };
            (wrapper, actor_child)
        };

        // Спавним на стратегической позиции (StrategicPosition → world coordinates)
        let spawn_pos = strategic_pos.to_world_position(0.5); // Y=0.5 (над землёй)
        actor_node.set_position(Vector3::new(spawn_pos.x, spawn_pos.y, spawn_pos.z));

        // КРИТИЧНО: Устанавливаем entity_id metadata для collision detection (shields, projectiles)
        let entity_id_variant = (entity.to_bits() as i64).to_variant();
        actor_node.set_meta("entity_id", &entity_id_variant);

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

        // КРИТИЧНО: Создаём unique shield material для каждого актора
        // (иначе все щиты будут share один material и гаснуть одновременно)
        if let Some(shield_sphere) = actor_node.try_get_node_as::<Node3D>("ShieldSphere") {
            if let Some(mut shield_mesh) = shield_sphere.try_get_node_as::<godot::classes::MeshInstance3D>("ShieldMesh") {
                // Получаем текущий material (shared SubResource)
                if let Some(shared_material) = shield_mesh.get_surface_override_material(0) {
                    // Clone material (создаём unique instance)
                    if let Some(duplicated) = shared_material.duplicate() {
                        // Cast Gd<Resource> → Gd<Material>
                        if let Ok(unique_material) = duplicated.try_cast::<godot::classes::Material>() {
                            shield_mesh.set_surface_override_material(0, &unique_material);

                            logger::log(&format!(
                                "🛡️ Created unique shield material for entity {:?}",
                                entity
                            ));
                        }
                    }
                }
            }
        }

        // AI state label (над головой, самый верхний)
        let mut ai_label = Label3D::new_alloc();
        let ai_text = format!("AI");
        ai_label.set_text(ai_text.as_str());
        ai_label.set_pixel_size(0.004);
        ai_label.set_billboard_mode(BillboardMode::ENABLED);
        ai_label.set_position(Vector3::new(0.0, 2.2, 0.0)); // Поднято с 1.4 до 2.2
        ai_label.set_modulate(Color::from_rgb(0.8, 0.8, 0.2)); // Желтый
        actor_node.add_child(&ai_label.clone().upcast::<Node>());

        // Health label под AI
        let mut health_label = Label3D::new_alloc();
        let health_text = format!("HP: {}/{}", health.current, health.max);
        health_label.set_text(health_text.as_str());
        health_label.set_pixel_size(0.005);
        health_label.set_billboard_mode(BillboardMode::ENABLED);
        health_label.set_position(Vector3::new(0.0, 2.0, 0.0)); // Поднято с 1.2 до 2.0
        actor_node.add_child(&health_label.clone().upcast::<Node>());

        // Stamina label под health
        let mut stamina_label = Label3D::new_alloc();
        let stamina_text = format!("Stamina: {:.0}/{:.0}", stamina.current, stamina.max);
        stamina_label.set_text(stamina_text.as_str());
        stamina_label.set_pixel_size(0.004);
        stamina_label.set_billboard_mode(BillboardMode::ENABLED);
        stamina_label.set_position(Vector3::new(0.0, 1.8, 0.0)); // Поднято с 1.0 до 1.8
        stamina_label.set_modulate(Color::from_rgb(0.2, 0.8, 0.2)); // Зелёный
        actor_node.add_child(&stamina_label.clone().upcast::<Node>());

        // Shield label под stamina (только если есть EnergyShield компонент)
        let shield_label_opt = if let Some(shield) = shield_opt {
            let mut shield_label = Label3D::new_alloc();
            let shield_text = format!("Shield: {:.0}/{:.0}", shield.current_energy, shield.max_energy);
            shield_label.set_text(shield_text.as_str());
            shield_label.set_pixel_size(0.004);
            shield_label.set_billboard_mode(BillboardMode::ENABLED);
            shield_label.set_position(Vector3::new(0.0, 1.6, 0.0)); // Поднято с 0.8 до 1.6
            shield_label.set_modulate(Color::from_rgb(0.3, 0.6, 1.0)); // Синий (как щит)
            actor_node.add_child(&shield_label.clone().upcast::<Node>());
            Some(shield_label)
        } else {
            None
        };

        // Добавляем в сцену через SceneRoot (СНАЧАЛА добавляем в дерево!)
        // ВАЖНО: добавляем scene_node (может быть wrapper или actor напрямую)
        let mut root = scene_root.node.clone();
        root.add_child(&scene_node.clone().upcast::<Node>());

        // КРИТИЧНО: Устанавливаем collision layers явно (даже если есть в TSCN)
        // Actors (layer 2) коллидируют с Actors + Environment (layers 2,3)
        if let Ok(mut char_body) = actor_node.clone().try_cast::<godot::classes::CharacterBody3D>() {
            char_body.set_collision_layer(crate::shared::collision::COLLISION_LAYER_ACTORS);
            char_body.set_collision_mask(crate::shared::collision::COLLISION_MASK_ACTORS);
            logger::log("  → Collision layers set: Actors (layer 2, mask 2|4)");
        }

        // Setup ShieldSphere initial state (collision + visibility)
        if let Some(mut shield_sphere) = actor_node.try_get_node_as::<godot::classes::StaticBody3D>("ShieldSphere") {
            if let Some(shield) = shield_opt {
                // Есть EnergyShield компонент → устанавливаем collision state по is_active
                let collision_layer = if shield.is_active() {
                    crate::shared::collision::COLLISION_LAYER_SHIELDS // 16
                } else {
                    0 // No collision when depleted
                };
                shield_sphere.set_collision_layer(collision_layer);
                shield_sphere.set_collision_mask(crate::shared::collision::COLLISION_MASK_SHIELDS); // 0 (passive)
                shield_sphere.set_visible(true); // Визуал активен (shader контролирует transparency)
                logger::log(&format!(
                    "  → ShieldSphere initialized: is_active={}, collision_layer={}",
                    shield.is_active(), collision_layer
                ));
            } else {
                // Нет EnergyShield компонента → отключаем полностью
                shield_sphere.set_visible(false); // Скрываем визуал
                shield_sphere.set_collision_layer(0); // No collision
                logger::log("  → ShieldSphere hidden (no EnergyShield component)");
            }
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
        logger::log("  → NavigationAgent3D added (avoidance enabled)");

        // Создаём AvoidanceReceiver для обработки velocity_computed signal
        let mut avoidance_receiver = Gd::<crate::navigation::AvoidanceReceiver>::from_init_fn(|base| {
            crate::navigation::AvoidanceReceiver::init(base)
        });
        avoidance_receiver.set_name("AvoidanceReceiver");

        // Устанавливаем entity_id (для callback → ECS event)
        avoidance_receiver.bind_mut().entity_id = entity.to_bits() as i64;

        // Устанавливаем путь к SimulationBridge (для EventWriter доступа)
        let bridge_path = root.get_path();
        avoidance_receiver.bind_mut().simulation_bridge_path = bridge_path;

        // Добавляем как child actor_node
        actor_node.add_child(&avoidance_receiver.upcast::<Node>());
        logger::log("  → AvoidanceReceiver added (velocity_computed signal)");

        // Регистрируем в VisualRegistry (Entity → Godot Node + reverse mapping)
        let instance_id = actor_node.instance_id();
        visuals.visuals.insert(entity, actor_node.clone());
        visuals.node_to_entity.insert(instance_id, entity);
        visuals.health_labels.insert(entity, health_label);
        visuals.stamina_labels.insert(entity, stamina_label);
        visuals.ai_state_labels.insert(entity, ai_label);
        if let Some(shield_label) = shield_label_opt {
            visuals.shield_labels.insert(entity, shield_label);
        }

        // КРИТИЧНО: actor_node теперь САМ CharacterBody3D
        // Mapping InstanceId → Entity происходит через visuals.node_to_entity (выше)
        logger::log(&format!("  → CharacterBody3D (root) mapped for entity {:?}", entity));

        // КРИТИЧНО: PostSpawn коррекция — отправляем точную позицию обратно в ECS
        let final_position = actor_node.get_global_position();
        transform_events.write(voidrun_simulation::ai::GodotTransformEvent::PostSpawn {
            entity,
            position: Vec3::new(final_position.x, final_position.y, final_position.z),
        });

        logger::log(&format!("✅ Spawned visual (prefab: {}) at strategic {:?}", prefab_path.path, strategic_pos));
    }
}
