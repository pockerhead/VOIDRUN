//! Tests for damage systems.

#[cfg(test)]
mod tests {
    use crate::components::Stamina;
    use crate::combat::{DamageDealt, EntityDied, DamageSource, AppliedDamage};
    use bevy::prelude::*;
    use super::super::damage::calculate_damage;

    #[test]
    fn test_damage_calculation_full_stamina() {
        let stamina = Stamina::new(100.0); // 100% stamina
        let damage = calculate_damage(20, Some(&stamina), None);

        // 100% stamina → 1.0x multiplier → 20 damage
        assert_eq!(damage, 20);
    }

    #[test]
    fn test_damage_calculation_half_stamina() {
        let mut stamina = Stamina::new(100.0);
        stamina.consume(50.0); // 50% stamina

        let damage = calculate_damage(20, Some(&stamina), None);

        // 50% stamina → sqrt(0.5) = 0.707 → ~14 damage
        assert!(damage >= 14 && damage <= 15, "damage = {}", damage);
    }

    #[test]
    fn test_damage_calculation_low_stamina() {
        let mut stamina = Stamina::new(100.0);
        stamina.consume(75.0); // 25% stamina

        let damage = calculate_damage(20, Some(&stamina), None);

        // 25% stamina → sqrt(0.25) = 0.5 → 10 damage
        assert_eq!(damage, 10);
    }

    #[test]
    fn test_damage_calculation_no_stamina() {
        let damage = calculate_damage(20, None, None);

        // Без stamina компонента → full damage
        assert_eq!(damage, 20);
    }

    #[test]
    fn test_damage_dealt_event() {
        let event = DamageDealt {
            attacker: Entity::PLACEHOLDER,
            target: Entity::PLACEHOLDER,
            damage: 15,
            source: DamageSource::Melee,
            applied_damage: AppliedDamage::Direct,
            impact_point: Vec3::ZERO,
            impact_normal: Vec3::Z,
        };

        assert_eq!(event.damage, 15);
        assert_eq!(event.source, DamageSource::Melee);
        assert_eq!(event.applied_damage, AppliedDamage::Direct);
    }

    #[test]
    fn test_entity_died_event() {
        let event = EntityDied {
            entity: Entity::PLACEHOLDER,
            killer: Some(Entity::PLACEHOLDER),
        };

        assert!(event.killer.is_some());
    }
}
