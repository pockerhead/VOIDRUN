//! Label synchronization systems (health, stamina, shield, AI state)

use bevy::prelude::*;
use voidrun_simulation::{Health, Stamina};
use voidrun_simulation::ai::AIState;
use crate::shared::VisualRegistry;

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

/// Sync shield energy changes → Godot Label3D
///
/// NAMING: `_main_thread` суффикс = Godot API calls (NonSend resources)
pub fn sync_shield_labels_main_thread(
    query: Query<(Entity, &voidrun_simulation::components::EnergyShield), Changed<voidrun_simulation::components::EnergyShield>>,
    mut visuals: NonSendMut<VisualRegistry>,
) {
    for (entity, shield) in query.iter() {
        let Some(label) = visuals.shield_labels.get_mut(&entity) else {
            continue;
        };

        let text = format!("Shield: {:.0}/{:.0}", shield.current_energy, shield.max_energy);
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
