//! Combat system module (Godot-driven combat architecture)
//!
//! ECS ответственность:
//! - Game state: Health, Stamina, Attacker stats
//! - Combat rules: damage calculation, stamina costs
//! - Events: DamageDealt, EntityDied
//!
//! Godot ответственность:
//! - AnimationTree: weapon swing timing
//! - Area3D hitbox: collision detection
//! - GodotCombatEvent: WeaponHit → ECS damage calculation
//!
//! Архитектура: docs/decisions/ADR-003-ecs-vs-godot-physics-ownership.md

use bevy::prelude::*;

pub mod damage;
pub mod melee;
pub mod stamina;
pub mod weapon;
pub mod weapon_stats;

// Re-export основных типов
pub use damage::{DamageDealt, DamageSource, EntityDied, Dead, DespawnAfter, calculate_damage};
pub use melee::{
    MeleeAttackState, AttackPhase, MeleeAttackIntent, MeleeAttackStarted, MeleeHit,
    MeleeAttackType, push_melee_hit, drain_melee_hit_queue,
    ai_melee_attack_intent, start_melee_attacks, update_melee_attack_phases, process_melee_hits,
};
pub use stamina::{Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST};
pub use weapon::{WeaponFired, WeaponFireIntent, ProjectileHit};
pub use weapon_stats::{WeaponStats, WeaponType, update_weapon_cooldowns};

/// Combat Plugin (Godot-driven architecture)
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
            .add_event::<MeleeAttackIntent>()
            .add_event::<MeleeAttackStarted>()
            .add_event::<MeleeHit>();

        // Регистрация систем в FixedUpdate
        app.add_systems(
            FixedUpdate,
            (
                // Фаза 1: Cooldowns (unified weapon cooldowns)
                update_weapon_cooldowns,

                // Фаза 2: Attack intent generation (ECS strategic decision)
                // Godot tactical validation в process_*_intents_main_thread
                weapon::ai_weapon_fire_intent,
                ai_melee_attack_intent,

                // Фаза 3: Attack execution (start attacks from approved intents)
                start_melee_attacks,
                update_melee_attack_phases,

                // Фаза 4: Damage application (from Godot events + projectiles + melee hits)
                damage::apply_damage,
                weapon::process_projectile_hits,
                process_melee_hits,

                // Фаза 5: Death handling
                damage::disable_ai_on_death,
                damage::despawn_after_timeout,

                // Фаза 6: Stamina management
                stamina::regenerate_stamina,
                stamina::detect_exhaustion,

                // Projectile cleanup — в Godot (GodotProjectile::_physics_process)
            )
                .chain(), // Последовательное выполнение
        );
    }
}
