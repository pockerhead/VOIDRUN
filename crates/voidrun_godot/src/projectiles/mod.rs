//! Projectiles domain — Godot-managed projectile physics and collision detection.
//!
//! # Architecture
//!
//! This domain handles projectile lifecycle entirely within Godot:
//! - **projectile**: GodotProjectile node (Area3D signal-based collision)
//! - **registry**: GodotProjectileRegistry (tracking projectiles for ECS collision processing)
//!
//! # Design Rationale (ADR-005)
//!
//! Projectiles are Godot tactical layer concern:
//! - Godot owns entire lifecycle: spawn, physics, collision detection, cleanup
//! - ECS receives only ProjectileHit events for damage calculation
//! - Projectiles NOT stored in ECS (tactical layer only)
//!
//! # Collision Model
//!
//! - Area3D signals (area_entered, body_entered)
//! - Collision info stored in projectile until ECS processing
//! - Separate shield vs body collision handling
//!
//! # Submodules
//!
//! - `projectile`: GodotProjectile node + collision structs
//! - `registry`: GodotProjectileRegistry resource

pub mod projectile;
pub mod registry;

// Re-export projectile node
pub use projectile::GodotProjectile;

// Re-export registry
pub use registry::GodotProjectileRegistry;
