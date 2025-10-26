use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod schedules;
mod input;
mod player;

// Domain modules (БЕЗ systems/ папки!)
mod shared;
mod visual_sync;
mod combat;          // UNIFIED: melee + ai_melee + ranged
mod navigation;      // Obstacle avoidance + navmesh baking + events
mod projectiles;     // Godot-managed projectile physics + collision
mod ui;              // Debug overlays + in-game UI
mod player_shooting; // Player ADS + Hip Fire mechanics
mod shield_vfx;
mod attachment;
mod vision;
mod weapon_switch;
mod movement;        // Movement commands + navigation + velocity

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
