//! Damage calculation система (Godot-driven combat)
//!
//! ECS ответственность:
//! - Damage calculation с модификаторами (stamina multiplier)
//! - Health application
//! - Death detection
//!
//! Godot отправляет: GodotCombatEvent::WeaponHit → apply_damage
//! ECS отправляет: DamageDealt, EntityDied events

use bevy::prelude::*;
use crate::components::{Health, Stamina};
use crate::combat::Attacker;

/// Событие: урон нанесен
///
/// Генерируется после применения damage к Health.
/// Используется для UI, звуков, эффектов.
#[derive(Event, Debug, Clone)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: u32,
    pub target_died: bool,
}

/// Событие: entity умер (health <= 0)
#[derive(Event, Debug, Clone)]
pub struct EntityDied {
    pub entity: Entity,
    pub killer: Option<Entity>,
}

/// Компонент-маркер: entity мертв (Health <= 0)
///
/// Используется для визуальных эффектов (death animation, fade-out).
/// Деспавн не автоматический — трупы остаются на месте.
#[derive(Component, Debug)]
pub struct Dead;

/// Система: apply damage (placeholder для Godot-driven combat)
///
/// TODO: Будет читать GodotCombatEvent::WeaponHit когда Godot integration готов
/// Сейчас: stub system для компиляции
pub fn apply_damage(
    mut _damage_dealt_events: EventWriter<DamageDealt>,
    mut _entity_died_events: EventWriter<EntityDied>,
    mut _targets: Query<(&mut Health, Option<&Stamina>)>,
    _attackers: Query<(&Attacker, &Stamina)>,
) {
    // TODO: Читать GodotCombatEvent::WeaponHit events
    // Godot AnimationTree trigger hitbox → WeaponHit { attacker, target } → apply_damage
    //
    // for event in godot_combat_events.read() {
    //     match event {
    //         GodotCombatEvent::WeaponHit { attacker, target } => {
    //             apply_weapon_hit(*attacker, *target, &mut targets, &attackers, ...);
    //         }
    //     }
    // }

    // Stub для компиляции
    // Реальная логика будет после Godot integration
}

/// Вычисляет final damage с модификаторами
///
/// Формула:
/// - Base damage × stamina_multiplier(attacker)
/// - stamina_multiplier = sqrt(stamina_percent)
///   - 100% stamina → 1.0x damage
///   - 50% stamina → 0.707x damage
///   - 25% stamina → 0.5x damage
///
/// Таким образом низкая stamina attacker наносит меньше урона.
pub fn calculate_damage(
    base_damage: u32,
    attacker_stamina: Option<&Stamina>,
    _target_stamina: Option<&Stamina>, // Для будущих defense модификаторов
) -> u32 {
    let mut final_damage = base_damage as f32;

    // Stamina multiplier для attacker
    if let Some(stamina) = attacker_stamina {
        let stamina_percent = stamina.current / stamina.max;
        let multiplier = stamina_percent.sqrt(); // sqrt для мягкого scaling
        final_damage *= multiplier;
    }

    // TODO: Target armor/defense модификаторы

    final_damage.round() as u32
}

/// Система: отключение AI при смерти
///
/// Убирает AIState и MovementCommand компоненты у мертвых entities.
/// Добавляет маркер Dead для визуальных эффектов.
pub fn disable_ai_on_death(
    mut commands: Commands,
    mut death_events: EventReader<EntityDied>,
) {
    for event in death_events.read() {
        // Удаляем AI компоненты через Commands
        if let Ok(mut entity_commands) = commands.get_entity(event.entity) {
            entity_commands.remove::<crate::ai::AIState>();
            entity_commands.remove::<crate::components::MovementCommand>();
            entity_commands.insert(Dead);

            crate::log(&format!("INFO: Disabled AI for dead entity {:?}", event.entity));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

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
            target_died: false,
        };

        assert_eq!(event.damage, 15);
        assert!(!event.target_died);
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
