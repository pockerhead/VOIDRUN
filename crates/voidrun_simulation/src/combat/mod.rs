//! Combat system module
//!
//! Hitbox система, damage calculation, stamina, parry window.
//! Архитектура: docs/architecture/physics-architecture.md (раздел Combat)

use bevy::prelude::*;

pub mod hitbox;
pub mod damage;
pub mod stamina;
pub mod weapon;

// Re-export основных типов
pub use hitbox::{AttackHitbox, Attacker, AttackStarted, HitboxOverlap};
pub use damage::{DamageDealt, EntityDied, Dead, calculate_damage};
pub use stamina::{Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST};
pub use weapon::{Weapon, WeaponState, spawn_weapon, collision};

/// Combat Plugin
///
/// Регистрирует все combat системы в FixedUpdate для детерминизма.
/// Порядок выполнения:
/// 1. tick_attack_cooldowns — обновление cooldown таймеров
/// 2. spawn_attack_hitbox — спавн hitbox при AttackStarted событиях
/// 3. detect_hitbox_overlaps — проверка overlap → HitboxOverlap события
/// 4. apply_damage — обработка HitboxOverlap → damage → DamageDealt/EntityDied
/// 5. consume_stamina_on_attack — вычитание stamina за атаки
/// 6. regenerate_stamina — восстановление stamina
/// 7. detect_exhaustion — exhaustion status management
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Регистрация событий
        app.add_event::<AttackStarted>()
            .add_event::<HitboxOverlap>()
            .add_event::<DamageDealt>()
            .add_event::<EntityDied>();

        // Регистрация систем в FixedUpdate
        app.add_systems(
            FixedUpdate,
            (
                // Фаза 0: Weapon attachment (для новых actors)
                weapon::attach_weapons_to_actors,

                // Фаза 1: Cooldowns и input
                hitbox::tick_attack_cooldowns,

                // Фаза 2: Hitbox spawn и detection (legacy system, позже удалим)
                hitbox::spawn_attack_hitbox,
                hitbox::detect_hitbox_overlaps,

                // Фаза 2.5: Weapon swing triggering
                weapon::trigger_weapon_swing,
                weapon::weapon_swing_animation,

                // Фаза 3: Weapon collision detection (новая система)
                weapon::weapon_collision_detection,

                // Фаза 4: Damage application
                damage::apply_damage,

                // Фаза 5: Death handling (AI отключение, деспавна нет — трупы остаются)
                damage::disable_ai_on_death,

                // Фаза 6: Stamina management
                stamina::consume_stamina_on_attack,
                stamina::regenerate_stamina,
                stamina::detect_exhaustion,
            )
                .chain(), // Последовательное выполнение для детерминизма
        );
    }
}
