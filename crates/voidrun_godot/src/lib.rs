use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod systems;
mod projectile;
mod chunk_navmesh;
mod avoidance_receiver;
mod events;
mod los_helpers;
pub mod collision_layers;

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
