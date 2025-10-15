//! Unified weapon stats component (melee + ranged)
//!
//! Architecture Decision:
//! - `Attacker` + `Weapon` объединены в `WeaponStats`
//! - `WeaponType` enum: Melee / Ranged / Hybrid
//! - Required component: `Attachment` (визуальный prefab)
//!
//! Rationale:
//! - Единый источник истины для weapon data
//! - Легко swapить оружие (одна замена компонента)
//! - Hybrid weapons работают из коробки
//! - Меньше boilerplate кода
//!
//! Trade-offs:
//! - Unused поля (melee не использует `range`, ranged не использует `windup_duration`)
//! - Acceptable: memory footprint минимален (несколько f32 полей)

use bevy::prelude::*;
use crate::Attachment;

/// Weapon stats component (melee + ranged)
///
/// Unified component для всех типов оружия:
/// - Melee (мечи, топоры)
/// - Ranged (пистолеты, винтовки)
/// - Hybrid (штык-ножи)
///
/// Архитектура:
/// - ECS хранит: stats, cooldown (game state)
/// - Godot выполняет: animation, hitbox, projectile spawn
/// - Events: WeaponFired/MeleeHit (Godot → ECS)
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
#[require(Attachment)]  // Weapon всегда имеет визуальный prefab
pub struct WeaponStats {
    /// Тип оружия
    pub weapon_type: WeaponType,

    /// Базовый урон (без модификаторов)
    pub base_damage: u32,

    /// Cooldown между атаками (секунды)
    pub attack_cooldown: f32,

    /// Текущий cooldown timer (уменьшается до 0)
    pub cooldown_timer: f32,

    // === Melee-specific stats ===
    /// Радиус атаки для melee (метры)
    pub attack_radius: f32,

    /// Windup duration (замах) (секунды)
    pub windup_duration: f32,

    /// Active phase duration (удар) (секунды)
    pub attack_duration: f32,

    /// Recovery duration (восстановление) (секунды)
    pub recovery_duration: f32,

    /// Parry window duration (окно парирования) (секунды)
    ///
    /// First portion of Active phase where attacker can be parried.
    /// Hitbox is disabled during this window.
    pub parry_window: f32,

    /// Parry active window duration (seconds)
    ///
    /// How long defender's parry window stays active after pressing parry button.
    /// This is the window where defender can successfully parry incoming attacks.
    pub parry_active_duration: f32,

    /// Stagger duration after being parried (seconds)
    ///
    /// How long attacker is stunned after being successfully parried.
    /// During stagger, attacker cannot perform any actions.
    pub stagger_duration: f32,

    // === Ranged-specific stats ===
    /// Дальность выстрела (метры)
    pub range: f32,

    /// Скорость projectile (м/с)
    pub projectile_speed: f32,

    /// Радиус слышимости выстрела (метры)
    pub hearing_range: f32,
}

/// Тип оружия
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum WeaponType {
    /// Melee weapon (мечи, топоры)
    Melee {
        /// Может ли блокировать
        can_block: bool,
        /// Может ли парировать
        can_parry: bool,
    },
    /// Ranged weapon (пистолеты, винтовки)
    Ranged,
    /// Hybrid (штык-нож) — может и melee и ranged
    Hybrid,
}

impl Default for WeaponStats {
    fn default() -> Self {
        Self::melee_sword()
    }
}

impl WeaponStats {
    /// Создать melee weapon (меч)
    pub fn melee_sword() -> Self {
        Self {
            weapon_type: WeaponType::Melee {
                can_block: true,
                can_parry: true,
            },
            base_damage: 25,
            attack_cooldown: 1.0,
            cooldown_timer: 0.0,

            // Melee stats
            attack_radius: 2.0,
            windup_duration: 0.3,
            attack_duration: 0.3,
            recovery_duration: 0.3,
            parry_window: 0.1,              // 33% of 0.3s attack
            parry_active_duration: 0.2,     // 200ms parry window for defender
            stagger_duration: 1.5,          // 1.5s stun after being parried

            // Ranged stats (unused для melee)
            range: 0.0,
            projectile_speed: 0.0,
            hearing_range: 0.0,
        }
    }

    /// Создать ranged weapon (пистолет)
    pub fn ranged_pistol() -> Self {
        Self {
            weapon_type: WeaponType::Ranged,
            base_damage: 10,
            attack_cooldown: 0.5,
            cooldown_timer: 0.0,

            // Melee stats (unused для ranged)
            attack_radius: 0.0,
            windup_duration: 0.0,
            attack_duration: 0.0,
            recovery_duration: 0.0,
            parry_window: 0.0,
            parry_active_duration: 0.0,
            stagger_duration: 0.0,

            // Ranged stats
            range: 20.0,
            projectile_speed: 300.0,
            hearing_range: 100.0,
        }
    }

    /// Может ли weapon атаковать (cooldown == 0)
    pub fn can_attack(&self) -> bool {
        self.cooldown_timer <= 0.0
    }

    /// Начать cooldown после атаки
    pub fn start_cooldown(&mut self) {
        self.cooldown_timer = self.attack_cooldown;
    }

    /// Это melee weapon?
    pub fn is_melee(&self) -> bool {
        matches!(
            self.weapon_type,
            WeaponType::Melee { .. } | WeaponType::Hybrid
        )
    }

    /// Это ranged weapon?
    pub fn is_ranged(&self) -> bool {
        matches!(self.weapon_type, WeaponType::Ranged | WeaponType::Hybrid)
    }

    /// Может ли weapon блокировать?
    pub fn can_block(&self) -> bool {
        match self.weapon_type {
            WeaponType::Melee { can_block, .. } => can_block,
            _ => false,
        }
    }

    /// Может ли weapon парировать?
    pub fn can_parry(&self) -> bool {
        match self.weapon_type {
            WeaponType::Melee { can_parry, .. } => can_parry,
            _ => false,
        }
    }
}

/// System: обновление weapon cooldowns
pub fn update_weapon_cooldowns(
    mut weapons: Query<&mut WeaponStats>,
    time: Res<Time>,
) {
    for mut weapon in weapons.iter_mut() {
        if weapon.cooldown_timer > 0.0 {
            weapon.cooldown_timer -= time.delta_secs();
            weapon.cooldown_timer = weapon.cooldown_timer.max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_stats_melee() {
        let weapon = WeaponStats::melee_sword();
        assert!(weapon.is_melee());
        assert!(!weapon.is_ranged());
        assert!(weapon.can_block());
        assert!(weapon.can_parry());
        assert_eq!(weapon.base_damage, 25);
        assert_eq!(weapon.attack_radius, 2.0);
    }

    #[test]
    fn test_weapon_stats_ranged() {
        let weapon = WeaponStats::ranged_pistol();
        assert!(!weapon.is_melee());
        assert!(weapon.is_ranged());
        assert!(!weapon.can_block());
        assert!(!weapon.can_parry());
        assert_eq!(weapon.base_damage, 10);
        assert_eq!(weapon.range, 20.0);
    }

    #[test]
    fn test_weapon_cooldown() {
        let mut weapon = WeaponStats::melee_sword();
        assert!(weapon.can_attack());

        weapon.start_cooldown();
        assert!(!weapon.can_attack());
        assert_eq!(weapon.cooldown_timer, 1.0);

        // Simulate tick
        weapon.cooldown_timer -= 0.5;
        assert!(!weapon.can_attack());

        weapon.cooldown_timer -= 0.5;
        assert!(weapon.can_attack());
    }
}
