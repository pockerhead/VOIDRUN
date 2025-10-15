//! Базовые компоненты акторов: Actor, Health, Stamina

use bevy::prelude::*;

/// Актор (NPC, игрок, враг) — базовый компонент для живых существ
///
/// Автоматически добавляет Health, Stamina, StrategicPosition, PrefabPath через Required Components.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
#[require(Health, Stamina, crate::components::StrategicPosition, crate::components::PrefabPath)]
pub struct Actor {
    /// Stable ID фракции (для reputation, diplomacy)
    pub faction_id: u64,
}

/// Здоровье актора
///
/// Инвариант: 0 ≤ current ≤ max
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100) // Default 100 HP
    }
}

impl Health {
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0
    }

    pub fn take_damage(&mut self, amount: u32) {
        self.current = self.current.saturating_sub(amount);
    }

    pub fn heal(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max);
    }
}

/// Выносливость (stamina) для атак/парирований
///
/// Инвариант: 0.0 ≤ current ≤ max
/// Regen: 10 units/sec default
/// Costs: attack 30, block 20
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Stamina {
    pub current: f32,
    pub max: f32,
    pub regen_rate: f32, // units per second
}

impl Default for Stamina {
    fn default() -> Self {
        Self::new(100.0) // Default 100 stamina
    }
}

impl Stamina {
    pub fn new(max: f32) -> Self {
        Self {
            current: max,
            max,
            regen_rate: 50.0, // 5x faster for testing combat
        }
    }

    pub fn can_afford(&self, cost: f32) -> bool {
        self.current >= cost
    }

    pub fn consume(&mut self, cost: f32) -> bool {
        if self.can_afford(cost) {
            self.current -= cost;
            true
        } else {
            false
        }
    }

    pub fn regenerate(&mut self, delta_time: f32) {
        self.current = (self.current + self.regen_rate * delta_time).min(self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_damage() {
        let mut health = Health::new(100);
        assert_eq!(health.current, 100);

        health.take_damage(30);
        assert_eq!(health.current, 70);
        assert!(health.is_alive());

        health.take_damage(100); // Saturating sub
        assert_eq!(health.current, 0);
        assert!(!health.is_alive());
    }

    #[test]
    fn test_health_heal() {
        let mut health = Health::new(100);
        health.take_damage(50);
        assert_eq!(health.current, 50);

        health.heal(30);
        assert_eq!(health.current, 80);

        health.heal(100); // Clamped to max
        assert_eq!(health.current, 100);
    }

    #[test]
    fn test_stamina_consume() {
        let mut stamina = Stamina::new(100.0);

        assert!(stamina.consume(30.0));
        assert_eq!(stamina.current, 70.0);

        assert!(!stamina.consume(80.0)); // Недостаточно
        assert_eq!(stamina.current, 70.0); // Не изменилась
    }

    #[test]
    fn test_stamina_regenerate() {
        let mut stamina = Stamina::new(100.0);
        stamina.consume(50.0);
        assert_eq!(stamina.current, 50.0);

        stamina.regenerate(2.0); // 2 sec × 10 units/sec = +20
        assert_eq!(stamina.current, 70.0);

        stamina.regenerate(10.0); // Clamp to max
        assert_eq!(stamina.current, 100.0);
    }
}
