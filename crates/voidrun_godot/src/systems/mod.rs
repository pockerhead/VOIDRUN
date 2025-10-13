pub mod visual_registry;
pub mod visual_sync;
pub mod attachment_system;
pub mod vision_system;
pub mod weapon_system;
pub mod movement_system;

pub use visual_registry::{VisualRegistry, AttachmentRegistry, SceneRoot};
pub use vision_system::VisionTracking;

pub use visual_sync::{
    spawn_actor_visuals_main_thread,
    sync_health_labels_main_thread,
    sync_stamina_labels_main_thread,
    sync_ai_state_labels_main_thread,
    disable_collision_on_death_main_thread,
    despawn_actor_visuals_main_thread,
    // УДАЛЕНО: sync_transforms_main_thread (ADR-005)
};

pub use attachment_system::{
    attach_prefabs_main_thread,
    detach_prefabs_main_thread,
};

pub use vision_system::{
    poll_vision_cones_main_thread,
};

pub use weapon_system::{
    weapon_aim_main_thread,
    process_weapon_fire_intents_main_thread,
    weapon_fire_main_thread,
    process_godot_projectile_hits,
};

pub use movement_system::{
    process_movement_commands_main_thread,
    apply_navigation_velocity_main_thread,
    // УДАЛЕНО: sync_strategic_position_from_godot (заменён на event-driven)
};

/// Godot delta time (обновляется каждый frame в SimulationBridge::process)
#[derive(bevy::prelude::Resource)]
pub struct GodotDeltaTime(pub f32);
