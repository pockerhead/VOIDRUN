//! Combat компоненты

use bevy::prelude::*;

/// Способность атаковать (melee/ranged)
#[derive(Component, Clone, Copy, Debug)]
pub struct Attacker {
    pub attack_cooldown: f32,
    pub cooldown_timer: f32,
    pub base_damage: u32,
    pub attack_radius: f32,
}

// NOTE: Weapon находится в crate::combat module (не дублируем здесь)
