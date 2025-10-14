//! Attachment system — dynamic TSCN prefab loading and attachment
//!
//! Architecture: ADR-007 (TSCN Prefabs + Dynamic Attachment) + ADR-004 (NonSend main thread systems)
//! - attach_prefabs_main_thread: Changed<Attachment> → load TSCN → attach (main thread only)
//! - detach_prefabs_main_thread: Query<DetachAttachment> → queue_free (main thread only)

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{PackedScene, Node3D};
use voidrun_simulation::{Attachment, DetachAttachment};
use crate::systems::{VisualRegistry, AttachmentRegistry};

/// Attach prefabs для новых Attachment компонентов
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn attach_prefabs_main_thread(
    query: Query<(Entity, &Attachment), Added<Attachment>>,
    visuals: NonSend<VisualRegistry>,
    mut attachments: NonSendMut<AttachmentRegistry>,
) {
    for (entity, attachment) in query.iter() {
        attach_single_prefab(entity, attachment, &visuals, &mut attachments);
    }
}

/// Detach prefabs по DetachAttachment marker component
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn detach_prefabs_main_thread(
    mut commands: Commands,
    query: Query<(Entity, &DetachAttachment)>,
    mut attachments: NonSendMut<AttachmentRegistry>,
) {
    for (entity, detach) in query.iter() {
        // Найти и удалить attachment
        let key = (entity, detach.attachment_point.clone());

        if let Some(mut attached_node) = attachments.attachments.remove(&key) {
            voidrun_simulation::log(&format!(
                "detach_prefabs: removing '{}' from entity {:?}",
                detach.attachment_point,
                entity
            ));
            attached_node.queue_free();
        }

        // Удалить marker component после обработки
        commands.entity(entity).remove::<DetachAttachment>();
    }
}

// === Helper functions ===

/// Attach single prefab to entity
fn attach_single_prefab(
    entity: Entity,
    attachment: &Attachment,
    visuals: &VisualRegistry,
    attachments: &mut AttachmentRegistry,
) {
    // 1. Найти host node
    let Some(host_node) = visuals.visuals.get(&entity) else {
        godot_warn!("attach_prefab: entity {:?} not in VisualRegistry", entity);
        return;
    };

    // 2. Найти attachment point
    let Some(mut attachment_point_node) = find_node_by_path(host_node, &attachment.attachment_point) else {
        voidrun_simulation::log_error(&format!(
            "attach_prefab: attachment point '{}' not found in entity {:?}",
            attachment.attachment_point,
            entity
        ));
        return;
    };

    // 3. Load TSCN prefab
    let prefab_scene = match load_packed_scene(&attachment.prefab_path) {
        Some(scene) => scene,
        None => {
            voidrun_simulation::log_error(&format!(
                "attach_prefab: failed to load prefab '{}' for entity {:?}",
                attachment.prefab_path,
                entity
            ));
            return;
        }
    };

    // 4. Instantiate prefab
    let prefab_instance = prefab_scene.instantiate_as::<Node3D>();

    // 5. Attach to attachment point
    attachment_point_node.add_child(&prefab_instance);

    // 6. Register in AttachmentRegistry
    let key = (entity, attachment.attachment_point.clone());
    attachments.attachments.insert(key, prefab_instance);

    voidrun_simulation::log(&format!(
        "attach_prefab: attached '{}' to entity {:?} at '{}'",
        attachment.prefab_path,
        entity,
        attachment.attachment_point
    ));
}

/// Load PackedScene from Godot resource path
fn load_packed_scene(path: &str) -> Option<Gd<PackedScene>> {
    let mut resource_loader = godot::classes::ResourceLoader::singleton();

    match resource_loader.load(path) {
        Some(resource) => resource.try_cast::<PackedScene>().ok(),
        None => {
            godot_error!("load_packed_scene: failed to load '{}'", path);
            None
        }
    }
}

/// Find child node by path (e.g. "RightHand/WeaponAttachment")
fn find_node_by_path(root: &Gd<Node3D>, path: &str) -> Option<Gd<Node3D>> {
    let node_path = NodePath::from(path);

    if root.has_node(&node_path) {
        Some(root.get_node_as::<Node3D>(&node_path))
    } else {
        None
    }
}
