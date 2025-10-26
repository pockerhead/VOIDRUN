//! Tests for stamina systems.

#[cfg(test)]
mod tests {
    use crate::components::Stamina;
    use crate::combat::{ATTACK_COST, BLOCK_COST, DODGE_COST};

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
