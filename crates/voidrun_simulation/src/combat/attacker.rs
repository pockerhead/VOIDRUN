//! Attacker component — характеристики атакующего актора
//!
//! Архитектура (Godot-driven combat):
//! - ECS хранит: base_damage, cooldown (game state)
//! - Godot выполняет: animation, hitbox collision detection
//! - Godot → ECS: GodotCombatEvent::WeaponHit → damage calculation

use bevy::prelude::*;

/// Attacker — компонент для акторов которые могут атаковать
///
/// Используется AI и damage calculation системами.
/// Weapon animation/hitbox управляется Godot AnimationTree.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Attacker {
    /// Базовый урон (без учёта stamina multiplier)
    pub base_damage: u32,

    /// Cooldown между атаками (секунды)
    pub attack_cooldown: f32,

    /// Текущий cooldown таймер (уменьшается до 0)
    pub cooldown_timer: f32,

    /// Радиус атаки (для AI target selection)
    pub attack_radius: f32,
}

impl Default for Attacker {
    fn default() -> Self {
        Self {
            base_damage: 25,
            attack_cooldown: 1.0,
            cooldown_timer: 0.0,
            attack_radius: 2.0,
        }
    }
}

impl Attacker {
    /// Может ли атаковать (cooldown == 0)
    pub fn can_attack(&self) -> bool {
        self.cooldown_timer <= 0.0
    }

    /// Начать атаку (сбросить cooldown)
    pub fn start_attack(&mut self) {
        self.cooldown_timer = self.attack_cooldown;
    }
}

/// System: обновление attack cooldown таймеров
pub fn tick_attack_cooldowns(
    mut query: Query<&mut Attacker>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for mut attacker in query.iter_mut() {
        if attacker.cooldown_timer > 0.0 {
            attacker.cooldown_timer -= delta;
            attacker.cooldown_timer = attacker.cooldown_timer.max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attacker_cooldown() {
        let mut attacker = Attacker::default();
        assert!(attacker.can_attack());

        attacker.start_attack();
        assert!(!attacker.can_attack());
        assert_eq!(attacker.cooldown_timer, 1.0);

        // Simulate tick
        attacker.cooldown_timer -= 0.5;
        assert!(!attacker.can_attack());

        attacker.cooldown_timer -= 0.5;
        assert!(attacker.can_attack());
    }
}
