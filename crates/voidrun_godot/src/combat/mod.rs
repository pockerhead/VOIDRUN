//! Combat domain — unified melee, AI decision-making, and ranged combat.
//!
//! # Architecture
//!
//! This domain combines three previously separate systems:
//! - **melee**: Melee attack execution (animations, hitboxes, parry)
//! - **ai_melee**: AI combat decision-making (unified attack/parry decisions)
//! - **ranged**: Ranged combat (targeting, firing, projectile physics)
//!
//! # Design Rationale
//!
//! These were combined because they share:
//! - Common combat state (WeaponStats, AttackPhase)
//! - Interrelated mechanics (melee parry vs ranged fire timing)
//! - Same tactical layer responsibilities (Godot validation + execution)
//!
//! # Submodules
//!
//! - `melee/`: Melee attack execution (Godot tactical layer)
//! - `ai_melee/`: AI unified combat decision system
//! - `ranged/`: Ranged weapon targeting, firing, projectile physics

pub mod melee;
pub mod ai_melee;
pub mod ranged;

// Re-export melee systems
pub use melee::{
    process_melee_attack_intents_main_thread,
    execute_melee_attacks_main_thread,
    poll_melee_hitboxes_main_thread,
    execute_parry_animations_main_thread,
    execute_stagger_animations_main_thread,
    detect_melee_windups_main_thread,
};

// Re-export AI combat decision system
pub use ai_melee::ai_melee_combat_decision_main_thread;

// Re-export ranged combat systems
pub use ranged::{
    // Targeting
    update_combat_targets_main_thread,
    weapon_aim_main_thread,
    // Firing
    process_ranged_attack_intents_main_thread,
    weapon_fire_main_thread,
    // Projectiles
    projectile_collision_system_main_thread,
    projectile_shield_collision_main_thread,
};
