use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod schedules;
mod projectile;
mod projectile_registry;
mod chunk_navmesh;
mod avoidance_receiver;
mod events;
mod los_helpers;
mod input;
mod player;
mod debug_overlay;
pub mod collision_layers;
pub mod actor_utils; // Actor spatial utilities (mutual facing, LOS, distance)

// Domain modules (БЕЗ systems/ папки!)
mod shared;
mod visual_sync;
mod melee;
mod shooting;
mod shield_vfx;
mod attachment;
mod vision;
mod weapon_switch;
mod movement_system; // TODO: переименовать в movement?
mod weapon_system;   // TODO: переименовать в weapon?
mod ai_melee_combat_decision; // TODO: переименовать в ai_combat?

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
