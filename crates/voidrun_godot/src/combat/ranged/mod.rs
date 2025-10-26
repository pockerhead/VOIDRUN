//! Weapon system - Godot визуализация (aim, fire, projectiles)
//!
//! Architecture (ADR-005):
//! - ECS: Weapon state (cooldown, fire decisions) → WeaponFired events
//! - Godot: Aim execution (bone rotation), Projectile spawn + physics
//! - Events: WeaponFired (ECS→Godot), ProjectileHit (Godot→ECS)

// Submodules
pub mod targeting;
pub mod ranged_attack;
pub mod projectile;

// Re-export systems
pub use targeting::{update_combat_targets_main_thread, weapon_aim_main_thread};
pub use ranged_attack::{process_ranged_attack_intents_main_thread, weapon_fire_main_thread};
pub use projectile::{
    projectile_collision_system_main_thread,
    projectile_shield_collision_main_thread,
};
