//! Stamina management система
//!
//! - Регенерация stamina (10 units/sec default)
//! - Attack costs (30 stamina)
//! - Block costs (20 stamina, будет в будущем)
//! - Exhaustion mechanic (опционально)

use bevy::prelude::*;
use crate::components::Stamina;
use crate::combat::hitbox::AttackStarted;

/// Стоимость различных действий (stamina points)
pub const ATTACK_COST: f32 = 30.0;
pub const BLOCK_COST: f32 = 20.0;
pub const DODGE_COST: f32 = 25.0; // Для будущего

/// Система: regenerate stamina для всех entities
///
/// Работает в FixedUpdate для детерминизма.
/// Regen rate берется из Stamina::regen_rate (default 10.0 units/sec).
pub fn regenerate_stamina(
    mut query: Query<&mut Stamina>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for mut stamina in query.iter_mut() {
        stamina.regenerate(delta);
    }
}

/// Система: consume stamina при атаках
///
/// Слушает AttackStarted события и вычитает ATTACK_COST из stamina attacker.
/// Если stamina недостаточно — атака не должна была начаться (проверка в AI/input).
pub fn consume_stamina_on_attack(
    mut attack_events: EventReader<AttackStarted>,
    mut attackers: Query<&mut Stamina>,
) {
    for event in attack_events.read() {
        if let Ok(mut stamina) = attackers.get_mut(event.attacker) {
            if !stamina.consume(ATTACK_COST) {
                eprintln!(
                    "WARN: Attack started with insufficient stamina: entity {:?} (current: {}, cost: {})",
                    event.attacker, stamina.current, ATTACK_COST
                );
            }
        }
    }
}

/// Exhaustion состояние (опционально)
///
/// Когда stamina падает ниже порога, entity получает debuff:
/// - Медленнее движение
/// - Меньше урона
/// - Дольше regen
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Exhausted {
    /// Movement speed multiplier (0.5 = half speed)
    pub movement_penalty: f32,
}

impl Default for Exhausted {
    fn default() -> Self {
        Self {
            movement_penalty: 0.7, // 30% slower
        }
    }
}

/// Система: detect exhaustion (stamina < 20%)
///
/// Добавляет Exhausted компонент когда stamina низкая.
/// Убирает когда восстановилась > 50%.
pub fn detect_exhaustion(
    mut commands: Commands,
    query: Query<(Entity, &Stamina, Option<&Exhausted>)>,
) {
    for (entity, stamina, exhausted) in query.iter() {
        let stamina_percent = stamina.current / stamina.max;

        if exhausted.is_none() && stamina_percent < 0.2 {
            // Стал exhausted
            commands.entity(entity).insert(Exhausted::default());
            // Debug logging
            // eprintln!("DEBUG: Entity {:?} is now exhausted (stamina: {:.1}%)", entity, stamina_percent * 100.0);
        } else if exhausted.is_some() && stamina_percent > 0.5 {
            // Восстановился
            commands.entity(entity).remove::<Exhausted>();
            // eprintln!("DEBUG: Entity {:?} recovered from exhaustion", entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stamina_regeneration_logic() {
        let mut stamina = Stamina::new(100.0);
        stamina.consume(50.0); // 50% stamina

        let delta = 1.0; // 1 second
        stamina.regenerate(delta);

        // После 1 sec: 50 + (10 * 1) = 60
        assert_eq!(stamina.current, 60.0);
    }

    #[test]
    fn test_stamina_attack_cost() {
        let mut stamina = Stamina::new(100.0);

        assert!(stamina.consume(ATTACK_COST));
        assert_eq!(stamina.current, 70.0);

        // Еще 2 атаки
        assert!(stamina.consume(ATTACK_COST));
        assert!(stamina.consume(ATTACK_COST));
        assert_eq!(stamina.current, 10.0);

        // Недостаточно для еще одной
        assert!(!stamina.consume(ATTACK_COST));
        assert_eq!(stamina.current, 10.0); // Не изменилась
    }

    #[test]
    fn test_exhaustion_threshold() {
        let stamina_high = Stamina { current: 50.0, max: 100.0, regen_rate: 10.0 };
        let stamina_low = Stamina { current: 15.0, max: 100.0, regen_rate: 10.0 };

        let high_percent = stamina_high.current / stamina_high.max;
        let low_percent = stamina_low.current / stamina_low.max;

        assert!(high_percent > 0.2); // Не exhausted
        assert!(low_percent < 0.2); // Exhausted
    }

    #[test]
    fn test_stamina_costs_constants() {
        assert_eq!(ATTACK_COST, 30.0);
        assert_eq!(BLOCK_COST, 20.0);
        assert_eq!(DODGE_COST, 25.0);
    }
}
