use godot::prelude::*;

mod simulation_bridge;
mod camera;

/// GDExtension entry point
struct VoidrunExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VoidrunExtension {}
