//! Visual registries — NonSend resources для Godot visual components
//!
//! Architecture: ADR-004 (NonSend resources, main thread only)
//! Shared by all visual sync systems (health/stamina/AI/attachments/etc.)

use bevy::prelude::*;
use godot::prelude::*;
use std::collections::HashMap;

/// Registry: маппинг Entity ↔ Godot visual components
///
/// NonSend resource — main thread only (Gd<T> не Send+Sync)
/// Используется всеми visual sync системами.
#[derive(Default)]
pub struct VisualRegistry {
    /// Main visual node (root для actor/ship/etc)
    pub visuals: HashMap<Entity, Gd<Node3D>>,

    /// Reverse mapping: Godot InstanceId → ECS Entity (для VisionCone overlaps)
    pub node_to_entity: HashMap<godot::prelude::InstanceId, Entity>,

    /// Health bar labels
    pub health_labels: HashMap<Entity, Gd<godot::classes::Label3D>>,

    /// Stamina bar labels
    pub stamina_labels: HashMap<Entity, Gd<godot::classes::Label3D>>,

    /// AI state labels
    pub ai_state_labels: HashMap<Entity, Gd<godot::classes::Label3D>>,
}

/// Registry: маппинг (Entity, attachment_point) → Godot Node3D (attached prefabs)
///
/// NonSend resource — main thread only (Gd<T> не Send+Sync)
/// Используется attachment системами.
#[derive(Default)]
pub struct AttachmentRegistry {
    pub attachments: HashMap<(Entity, String), Gd<Node3D>>,
}

/// Scene root — Godot scene Node3D для добавления визуальных child nodes
///
/// NonSend resource — main thread only (Gd<Node3D> не Send+Sync)
/// Инициализируется SimulationBridge и передаётся в spawn системы.
pub struct SceneRoot {
    pub node: Gd<Node3D>,
}
