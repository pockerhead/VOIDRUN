//! Navigation domain — obstacle avoidance, navmesh baking, and navigation events.
//!
//! # Architecture
//!
//! This domain handles Godot NavigationServer3D integration:
//! - **avoidance**: NavigationAgent3D signal handling (velocity_computed)
//! - **navmesh**: Runtime NavMesh baking for procgen chunks
//! - **events**: Navigation-specific Bevy events (SafeVelocityComputed)
//!
//! # Design Rationale
//!
//! Navigation is a Godot tactical layer concern:
//! - NavigationAgent3D for pathfinding + obstacle avoidance
//! - NavigationServer3D for runtime NavMesh baking (chunk streaming)
//! - Events bridge between Godot signals and ECS systems
//!
//! # Submodules
//!
//! - `avoidance`: AvoidanceReceiver node (NavigationAgent3D signal wrapper)
//! - `navmesh`: NavMesh runtime baking utilities (chunk-based procgen)
//! - `events`: SafeVelocityComputed event (Godot → ECS bridge)

pub mod avoidance;
pub mod navmesh;
pub mod events;

// Re-export avoidance receiver (Godot node)
pub use avoidance::AvoidanceReceiver;

// Re-export navmesh utilities
pub use navmesh::{NavMeshBakingParams, create_test_navigation_region_with_obstacles};

// Re-export events
pub use events::SafeVelocityComputed;
