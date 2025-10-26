//! Vision System — Godot VisionCone → GodotAIEvent
//!
//! Architecture: ADR-004 (NonSend resources, _main_thread naming), ADR-005 (Godot authoritative)
//! Poll-based: каждый frame проверяем overlapping_bodies в VisionCone → отправляем events

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{Area3D, Node};
use voidrun_simulation::ai::GodotAIEvent;
use crate::shared::VisualRegistry;
use std::collections::{HashMap, HashSet};

/// VisionTracking resource — кто кого видит (state для ActorSpotted/ActorLost events)
///
/// NonSend resource (HashMap<Entity, HashSet<Entity>>)
/// Key = observer entity, Value = set of spotted target entities
#[derive(Default)]
pub struct VisionTracking {
    pub spotted: HashMap<Entity, HashSet<Entity>>,
}

/// Poll VisionCone overlaps → отправка GodotAIEvent
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
/// Каждый frame проверяем Area3D.get_overlapping_bodies() → сравниваем с prev state → events
pub fn poll_vision_cones_main_thread(
    query: Query<Entity, With<voidrun_simulation::Actor>>,
    visuals: NonSend<VisualRegistry>,
    mut tracking: NonSendMut<VisionTracking>,
    mut ai_events: EventWriter<GodotAIEvent>,
) {

    for observer in query.iter() {
        let Some(observer_node) = visuals.visuals.get(&observer) else {
            continue;
        };
            // Находим VisionCone child
        let Some(vision_cone_node) = find_child_by_name(observer_node, "VisionCone") else {
            continue;
        };
        let Ok(area) = vision_cone_node.try_cast::<Area3D>() else {
            continue;
        };
        // Получаем overlapping bodies (Godot Array)
        let overlapping = area.get_overlapping_bodies();
        let mut current_spotted = HashSet::new();

        // Парсим overlapping bodies → находим entity targets
        for i in 0..overlapping.len() {
            if let Some(body) = overlapping.get(i) {
                let instance_id = body.instance_id();

                // Reverse lookup: Godot InstanceId → ECS Entity
                if let Some(&target_entity) = visuals.node_to_entity.get(&instance_id) {
                    // Не считаем себя
                    if target_entity != observer {
                        current_spotted.insert(target_entity);
                    }
                }
            }
        }

        // Сравниваем с prev state → генерируем events
        let prev_spotted = tracking.spotted.entry(observer).or_default().clone();

        // ActorSpotted: новые targets
        for target in current_spotted.difference(&prev_spotted) {
            ai_events.write(GodotAIEvent::ActorSpotted {
                observer,
                target: *target,
            });
        }

        // ActorLost: потерянные targets
        for target in prev_spotted.difference(&current_spotted) {
            ai_events.write(GodotAIEvent::ActorLost {
                observer,
                target: *target,
            });
        }

        // Обновляем tracking state
        *tracking.spotted.entry(observer).or_default() = current_spotted;
    }

}




/// Поиск child node по имени (рекурсивно)
fn find_child_by_name(parent: &Gd<Node3D>, name: &str) -> Option<Gd<Node>> {
    for i in 0..parent.get_child_count() {
        if let Some(child) = parent.get_child(i) {
            if child.get_name().to_string() == name {
                return Some(child);
            }
            // Рекурсивно ищем в детях
            if let Ok(child_node3d) = child.clone().try_cast::<Node3D>() {
                if let Some(found) = find_child_by_name(&child_node3d, name) {
                    return Some(found);
                }
            }
        }
    }
    None
}
