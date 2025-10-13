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

pub mod attacker;
pub mod damage;
pub mod stamina;
pub mod weapon;

// Re-export основных типов
pub use attacker::{Attacker, tick_attack_cooldowns};
pub use damage::{DamageDealt, EntityDied, Dead, DespawnAfter, calculate_damage};
pub use stamina::{Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST};
pub use weapon::{Weapon, WeaponFired, WeaponFireIntent, ProjectileHit};

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
            .add_event::<ProjectileHit>();

        // Регистрация систем в FixedUpdate
        app.add_systems(
            FixedUpdate,
            (
                // Фаза 1: Cooldowns (melee + weapon)
                tick_attack_cooldowns,
                weapon::update_weapon_cooldowns,

                // Фаза 2: Weapon fire intent (ECS strategic decision)
                // Godot tactical validation в process_weapon_fire_intents_main_thread
                weapon::ai_weapon_fire_intent,

                // Фаза 3: Damage application (from Godot events + projectiles)
                damage::apply_damage,
                weapon::process_projectile_hits,

                // Фаза 4: Death handling
                damage::disable_ai_on_death,
                damage::despawn_after_timeout,

                // Фаза 5: Stamina management
                stamina::regenerate_stamina,
                stamina::detect_exhaustion,

                // Projectile cleanup — в Godot (GodotProjectile::_physics_process)
            )
                .chain(), // Последовательное выполнение
        );
    }
}
