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
pub fn spawn_actor_visuals_main_thread(
    query: Query<(Entity, &Actor, &Health, &Stamina, &AIState), Added<Actor>>,
    mut visuals: NonSendMut<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
) {
    for (entity, actor, health, stamina, ai_state) in query.iter() {
        // Загружаем TSCN prefab из ассетов
        let mut loader = ResourceLoader::singleton();
        let path = "res://actors/test_actor.tscn";
        let scene = loader.load_ex(path).done();

        if scene.is_none() {
            voidrun_simulation::log("❌ Failed to load test_actor.tscn");
            continue;
        }

        let packed_scene: Gd<PackedScene> = scene.unwrap().cast();
        let instance = packed_scene.instantiate().unwrap();
        let mut actor_node = instance.cast::<Node3D>();

        // Цвет фракции — красим все MeshInstance3D дочерние ноды
        let faction_color = match actor.faction_id {
            1 => Color::from_rgb(0.2, 0.6, 1.0), // Blue
            2 => Color::from_rgb(0.8, 0.2, 0.2), // Red
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
        let ai_text = format!("[{:?}]", ai_state);
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

        // ПОСЛЕ добавления в SceneTree — добавляем NavigationAgent3D как прямой ребёнок actor_node
        // (NavigationAgent требует чтобы parent был уже в дереве сцены)
        let mut nav_agent = NavigationAgent3D::new_alloc();
        nav_agent.set_debug_enabled(true);
        nav_agent.set_debug_path_custom_color(Color::from_rgb(0.8, 0.2, 0.2));
        nav_agent.set_name("NavigationAgent3D"); // ВАЖНО: задаём имя явно!
        // Увеличенные distances чтобы актор не маятничил
        nav_agent.set_path_desired_distance(1.0);
        nav_agent.set_target_desired_distance(2.0); // Когда считать что достигли цели
        nav_agent.set_path_max_distance(3.0);

        // КРИТИЧНО: avoidance = false (мы не используем velocity_computed callback)
        // Наша архитектура: ECS → NavigationAgent.get_next_path_position() → CharacterBody
        // Без avoidance нет необходимости в velocity_computed signal
        nav_agent.set_avoidance_enabled(false);

        nav_agent.set_max_speed(1.5); // Соответствует MOVE_SPEED (для reference, не используется без avoidance)
        actor_node.add_child(&nav_agent.upcast::<Node>());
        voidrun_simulation::log("  → NavigationAgent3D added to actor_node (root)");

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

        voidrun_simulation::log(&format!("✅ Spawned visual (TSCN prefab) for entity {:?}", entity));
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
        if let Some(label) = visuals.health_labels.get_mut(&entity) {
            let text = format!("HP: {}/{}", health.current, health.max);
            label.set_text(text.as_str());
        }
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
        if let Some(label) = visuals.stamina_labels.get_mut(&entity) {
            let text = format!("Stamina: {:.0}/{:.0}", stamina.current, stamina.max);
            label.set_text(text.as_str());
        }
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
        if let Some(label) = visuals.ai_state_labels.get_mut(&entity) {
            let text = format!("[{:?}]", state);
            label.set_text(text.as_str());
        }
    }
}

/// Sync transform changes → Godot Node3D position
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn sync_transforms_main_thread(
    query: Query<(Entity, &Transform), (Changed<Transform>, Without<voidrun_simulation::MovementCommand>)>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    // Синхронизируем ECS Transform → Godot ТОЛЬКО для акторов БЕЗ MovementCommand
    // (акторы с MovementCommand управляются NavigationAgent, их позиция authoritative в Godot)
    for (entity, transform) in query.iter() {
        if let Some(node) = visuals.visuals.get_mut(&entity) {
            let pos = transform.translation;
            node.set_position(Vector3::new(pos.x, pos.y, pos.z));
        }
    }
}
