//! Combat domain module (domain-driven architecture)
//!
//! ECS ответственность:
//! - Game state: Health, Stamina, Weapon stats
//! - Combat rules: damage calculation, stamina costs
//! - Events: DamageDealt, EntityDied, MeleeHit, WeaponFired, etc.
//!
//! Godot ответственность:
//! - AnimationTree: weapon swing timing
//! - Area3D hitbox: collision detection
//! - GodotCombatEvent: WeaponHit → ECS damage calculation
//!
//! Архитектура: docs/decisions/ADR-003-ecs-vs-godot-physics-ownership.md

use bevy::prelude::*;

// Domain modules
pub mod components;
pub mod systems;
pub mod events;

// Re-export components
pub use components::{
    // Melee components
    MeleeAttackState, AttackPhase, ParryState, ParryPhase, StaggerState, ParryDelayTimer,
    MeleeAttackType,
    // Weapon component
    WeaponStats, WeaponType,
    // Stamina components
    Exhausted,
};

// Re-export events
pub use events::{
    // Melee events
    MeleeAttackIntent, MeleeAttackStarted, MeleeHit, ParryIntent, ParrySuccess,
    // Ranged events
    WeaponFireIntent, WeaponFired, ProjectileHit, ProjectileShieldHit,
    // Damage events
    DamageDealt, EntityDied, DamageSource, AppliedDamage,
    // Shared enums
    AttackType,
};

// Re-export systems
pub use systems::{
    // Melee systems
    start_melee_attacks, update_melee_attack_phases, process_melee_hits,
    start_parry, update_parry_states, update_stagger_states, process_parry_delay_timers,
    // Weapon systems
    update_weapon_cooldowns, ai_weapon_fire_intent,
    process_projectile_hits, process_projectile_shield_hits,
    // Damage systems
    Dead, DespawnAfter, apply_damage, calculate_damage, apply_damage_with_shield,
    shield_recharge_system, disable_ai_on_death, despawn_after_timeout,
    // Stamina systems
    ATTACK_COST, BLOCK_COST, DODGE_COST,
    regenerate_stamina, consume_stamina_on_attack, detect_exhaustion,
};

/// Combat Plugin (domain-driven architecture)
///
/// Регистрирует combat системы в FixedUpdate (64Hz).
///
/// Порядок выполнения:
/// 1. tick_attack_cooldowns — обновление cooldown таймеров
/// 2. apply_damage — обработка GodotCombatEvent → damage calculation
/// 3. disable_ai_on_death — отключение AI у мертвых
/// 4. regenerate_stamina — восстановление stamina
/// 5. detect_exhaustion — exhaustion status management
///
/// Godot отправляет GodotCombatEvent::WeaponHit → apply_damage → DamageDealt
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Регистрация событий
        app.add_event::<DamageDealt>()
            .add_event::<EntityDied>()
            .add_event::<WeaponFireIntent>()
            .add_event::<WeaponFired>()
            .add_event::<ProjectileHit>()
            .add_event::<ProjectileShieldHit>() // Shield collision events
            .add_event::<MeleeAttackIntent>()
            .add_event::<MeleeAttackStarted>()
            .add_event::<MeleeHit>()
            .add_event::<ParryIntent>()
            .add_event::<ParrySuccess>();

        // Регистрация систем в FixedUpdate
        app.add_systems(
            FixedUpdate,
            (
                // Фаза 1: Cooldowns (unified weapon cooldowns)
                update_weapon_cooldowns,

                // Фаза 2: Attack intent generation (ECS strategic decision)
                // Godot tactical validation в process_*_intents_main_thread
                ai_weapon_fire_intent,
                // NOTE: ai_melee_attack_intent REMOVED - replaced by unified ai_combat_decision_main_thread (in Godot layer)

                // Фаза 3: Attack execution (start attacks from approved intents)
                start_melee_attacks,
                update_melee_attack_phases,

                // Фаза 3.5: Parry system (defensive actions)
                process_parry_delay_timers, // Tick delay timers → generate ParryIntent
                start_parry,
                update_parry_states, // Includes parry success check at critical moment
                update_stagger_states,

                // Фаза 4: Damage application (from Godot events + projectiles + melee hits)
                apply_damage,
                process_projectile_hits,
                process_projectile_shield_hits, // Shield collision events → damage shield
                process_melee_hits,

                // Фаза 5: Death handling
                disable_ai_on_death,
                despawn_after_timeout,

                // Фаза 6: Stamina management + Shield recharge
                regenerate_stamina,
                detect_exhaustion,
                shield_recharge_system,

                // Projectile cleanup — в Godot (GodotProjectile::_physics_process)
            )
                .chain(), // Последовательное выполнение
        );
    }
}
