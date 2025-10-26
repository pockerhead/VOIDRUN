//! Tests for weapon systems.

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use crate::combat::{ProjectileHit, WeaponFireIntent};

    #[test]
    fn test_projectile_hit_event() {
        let shooter = Entity::PLACEHOLDER;
        let target = Entity::from_raw(1);

        let hit = ProjectileHit {
            shooter,
            target,
            damage: 20,
            impact_point: Vec3::ZERO,
            impact_normal: Vec3::Z,
        };

        assert_eq!(hit.shooter, shooter);
        assert_eq!(hit.damage, 20);
        assert_eq!(hit.impact_point, Vec3::ZERO);
    }

    #[test]
    fn test_weapon_fire_intent_event() {
        let shooter = Entity::PLACEHOLDER;
        let target = Entity::from_raw(1);

        let intent = WeaponFireIntent {
            shooter,
            target: Some(target),
            damage: 10,
            speed: 8.0,
            max_range: 20.0,
            hearing_range: 100.0,
        };

        assert_eq!(intent.shooter, shooter);
        assert_eq!(intent.target, Some(target));
        assert_eq!(intent.damage, 10);
    }
}
