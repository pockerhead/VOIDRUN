pub mod visual_registry;
pub mod visual_sync;
pub mod attachment_system;
pub mod vision_system;
pub mod weapon_system;
pub mod melee_system;
pub mod movement_system;
pub mod ai_melee_combat_decision;
pub mod player_camera_system;
pub mod weapon_switch;
pub mod player_shooting;

pub use visual_registry::{VisualRegistry, AttachmentRegistry, SceneRoot};
pub use vision_system::VisionTracking;

pub use visual_sync::{
    spawn_actor_visuals_main_thread,
    sync_health_labels_main_thread,
    sync_stamina_labels_main_thread,
    sync_ai_state_labels_main_thread,
    disable_collision_on_death_main_thread,
    despawn_actor_visuals_main_thread,
};

pub use attachment_system::{
    attach_prefabs_main_thread,
    detach_prefabs_main_thread,
};

pub use vision_system::{
    poll_vision_cones_main_thread,
};

pub use weapon_system::{
    update_combat_targets_main_thread,
    weapon_aim_main_thread,
    process_ranged_attack_intents_main_thread,
    weapon_fire_main_thread,
    projectile_collision_system_main_thread, // NEW: Event-driven projectile collision
};

pub use melee_system::{
    process_melee_attack_intents_main_thread,
    execute_melee_attacks_main_thread,
    poll_melee_hitboxes_main_thread,
    execute_parry_animations_main_thread,
    execute_stagger_animations_main_thread,
};

pub use ai_melee_combat_decision::{
    ai_melee_combat_decision_main_thread,
};

pub use movement_system::{
    apply_gravity_to_all_actors, // Gravity + jump для ВСЕХ акторов (ПЕРВАЯ!)
    process_movement_commands_main_thread,
    update_follow_entity_targets_main_thread,
    apply_retreat_velocity_main_thread,
    apply_navigation_velocity_main_thread,
    apply_safe_velocity_system, // NavigationAgent3D avoidance (velocity_computed signal)
    // УДАЛЕНО: sync_strategic_position_from_godot (заменён на event-driven)
};

pub use player_camera_system::{
    setup_player_camera,
    camera_toggle_system,
    player_mouse_look,
};

pub use weapon_switch::{
    process_player_weapon_switch,
    // process_weapon_switch удалён — теперь в voidrun_simulation::EquipmentPlugin
};

pub use player_shooting::{
    process_ads_toggle,
    update_ads_position_transition,
    player_hip_fire_aim,
};

/// Godot delta time (обновляется каждый frame в SimulationBridge::process)
#[derive(bevy::prelude::Resource)]
pub struct GodotDeltaTime(pub f32);
