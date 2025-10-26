//! Tests for WeaponStats component.

#[cfg(test)]
mod tests {
    use super::super::weapon::*;

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
