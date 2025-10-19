use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod schedules;
mod systems;
mod projectile;
mod chunk_navmesh;
mod avoidance_receiver;
mod events;
mod los_helpers;
mod input;
mod player;
mod debug_overlay;
pub mod collision_layers;
pub mod actor_utils; // Actor spatial utilities (mutual facing, LOS, distance)

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
