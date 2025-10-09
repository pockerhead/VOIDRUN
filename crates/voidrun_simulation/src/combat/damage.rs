//! Damage calculation система
//!
//! Обрабатывает HitboxOverlap события и применяет урон с модификаторами:
//! - Stamina multiplier: damage × sqrt(stamina_percent)
//! - Armor (в будущем)
//! - Critical hits (в будущем)

use bevy::prelude::*;
use crate::components::{Health, Stamina};
use crate::combat::hitbox::HitboxOverlap;

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

/// Система: apply damage от HitboxOverlap событий
///
/// 1. Читаем HitboxOverlap события
/// 2. Вычисляем final damage с модификаторами (stamina)
/// 3. Применяем damage к Health
/// 4. Генерируем DamageDealt и EntityDied события
pub fn apply_damage(
    mut overlap_events: EventReader<HitboxOverlap>,
    mut damage_dealt_events: EventWriter<DamageDealt>,
    mut entity_died_events: EventWriter<EntityDied>,
    mut targets: Query<(&mut Health, Option<&Stamina>)>,
    attackers: Query<&Stamina>, // Stamina attacker для модификатора
) {
    for overlap in overlap_events.read() {
        // Получаем target health
        let (mut target_health, target_stamina) = match targets.get_mut(overlap.target) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("WARN: HitboxOverlap: target {:?} has no Health component", overlap.target);
                continue;
            }
        };

        // Вычисляем final damage с stamina multiplier attacker
        let attacker_stamina = attackers.get(overlap.attacker).ok();
        let final_damage = calculate_damage(
            overlap.damage,
            attacker_stamina,
            target_stamina,
        );

        // Применяем damage
        let was_alive = target_health.is_alive();
        target_health.take_damage(final_damage);
        let is_alive = target_health.is_alive();

        // Debug logging
        // eprintln!("DEBUG: Damage applied: {:?} → {:?} ({} damage, health: {})",
        //     overlap.attacker, overlap.target, final_damage, target_health.current);

        // Событие: урон нанесен
        damage_dealt_events.send(DamageDealt {
            attacker: overlap.attacker,
            target: overlap.target,
            damage: final_damage,
            target_died: was_alive && !is_alive,
        });

        // Событие: entity умер
        if was_alive && !is_alive {
            entity_died_events.send(EntityDied {
                entity: overlap.target,
                killer: Some(overlap.attacker),
            });

            eprintln!("INFO: Entity {:?} killed by {:?}", overlap.target, overlap.attacker);
        }
    }
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
/// Убирает AIState и MovementInput компоненты у мертвых entities,
/// чтобы они перестали двигаться и атаковать.
/// Добавляет маркер Dead для визуальных эффектов.
pub fn disable_ai_on_death(
    mut commands: Commands,
    mut death_events: EventReader<EntityDied>,
    mut physics_query: Query<(&mut crate::components::PhysicsBody, Option<&mut crate::physics::MovementInput>)>,
) {
    for event in death_events.read() {
        // Обнуляем velocity и MovementInput сразу (не через Commands)
        if let Ok((mut physics, movement_input)) = physics_query.get_mut(event.entity) {
            physics.velocity = bevy::math::Vec3::ZERO;

            if let Some(mut input) = movement_input {
                input.direction = bevy::math::Vec3::ZERO;
            }
        }

        // Удаляем AI компоненты через Commands (задержка на 1 фрейм)
        if let Ok(mut entity_commands) = commands.get_entity(event.entity) {
            entity_commands.remove::<crate::ai::AIState>();
            entity_commands.remove::<crate::physics::MovementInput>();
            entity_commands.insert(Dead);

            eprintln!("INFO: Disabled AI for dead entity {:?}", event.entity);
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
