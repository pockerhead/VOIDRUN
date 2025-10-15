use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod systems;
mod projectile;
mod chunk_navmesh;
mod avoidance_receiver;
mod events;

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
