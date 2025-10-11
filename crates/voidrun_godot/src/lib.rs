use godot::prelude::*;

mod simulation_bridge;
mod camera;
mod systems;
mod projectile;

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
