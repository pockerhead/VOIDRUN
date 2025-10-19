//! Weapon system - events для ranged combat
//!
//! Architecture:
//! - ECS: WeaponStats (cooldown, decisions) в weapon_stats.rs
//! - Godot: Aim execution (bone rotation), Fire visual (spawn projectile)
//! - Events: WeaponFired (ECS→Godot), ProjectileHit (Godot→ECS)

use bevy::prelude::*;

// ❌ Projectile НЕ хранится в ECS — только в Godot (tactical layer)
// Godot полностью владеет lifecycle: spawn, physics, collision, cleanup
// ECS отвечает только за Weapon state и damage calculation

/// Event: Актёр ХОЧЕТ выстрелить (ECS strategic intent)
/// ECS принимает strategic decision: "cooldown готов, target в Combat state"
/// Godot validation проверяет tactical constraints: distance, LOS
///
/// **Note:** `target` опционален для player FPS shooting (direction = camera forward)
#[derive(Event, Debug, Clone)]
pub struct WeaponFireIntent {
    /// Кто хочет стрелять
    pub shooter: Entity,

    /// В кого хочет стрелять (None = player FPS shooting без target)
    pub target: Option<Entity>,

    /// Урон (из Weapon component)
    pub damage: u32,

    /// Скорость пули (из Weapon component)
    pub speed: f32,

    /// Max range (из Weapon component)
    pub max_range: f32,

    /// Радиус слышимости выстрела (для AI reaction)
    pub hearing_range: f32,
}

/// Event: Актёр стреляет (ECS → Godot, после validation)
/// Godot tactical layer проверил distance/LOS и разрешил выстрел
/// Godot рассчитывает точное direction из weapon bone (+Z axis)
///
/// **Note:** `target` опционален (None = player FPS shooting, direction = weapon forward)
#[derive(Event, Debug, Clone)]
pub struct WeaponFired {
    /// Кто стреляет
    pub shooter: Entity,

    /// В кого стреляет (None = направление из weapon bone, Some = fallback shooter→target)
    pub target: Option<Entity>,

    /// Урон пули
    pub damage: u32,

    /// Скорость пули
    pub speed: f32,

    /// Позиция стрелявшего (Godot Transform, для AI sound reaction)
    pub shooter_position: Vec3,

    /// Радиус слышимости выстрела (для AI reaction)
    pub hearing_range: f32,
}

/// Event: Projectile попал в цель (Godot → ECS)
#[derive(Event, Debug, Clone)]
pub struct ProjectileHit {
    /// Кто выстрелил (для предотвращения self-hit)
    pub shooter: Entity,

    /// В кого попали
    pub target: Entity,

    /// Урон
    pub damage: u32,
}


/// System: AI weapon fire intent (ECS strategic decision)
///
/// Архитектура (Hybrid Intent-based):
/// 1. ECS (strategic): Проверяет cooldown + AI state → генерирует WeaponFireIntent
/// 2. Godot (tactical): Проверяет distance/LOS → конвертирует Intent → WeaponFired
///
/// Почему так:
/// - ECS не знает точных Godot positions (только chunk-based StrategicPosition)
/// - Godot authoritative для tactical validation (distance, line of sight)
/// - Разделение ответственности: strategic intent vs tactical execution
pub fn ai_weapon_fire_intent(
    mut actors: Query<(Entity, &crate::ai::AIState, &mut crate::combat::WeaponStats)>,
    mut intent_events: EventWriter<WeaponFireIntent>,
) {
    use crate::ai::AIState;

    for (entity, state, mut weapon) in actors.iter_mut() {
        // Стреляем только в Combat state
        let AIState::Combat { target } = state else {
            continue;
        };

        // Только ranged weapons
        if !weapon.is_ranged() {
            continue;
        }

        // Проверяем cooldown (strategic constraint)
        if !weapon.can_attack() {
            continue;
        }

        // Генерируем intent (Godot проверит distance/LOS)
        intent_events.write(WeaponFireIntent {
            shooter: entity,
            target: Some(*target),
            damage: weapon.base_damage,
            speed: weapon.projectile_speed,
            max_range: weapon.range,
            hearing_range: weapon.hearing_range,
        });

        // Начинаем cooldown (ECS владеет cooldown state)
        weapon.start_cooldown();

        crate::log(&format!(
            "Actor {:?} wants to fire at {:?} (intent generated)",
            entity, target
        ));
    }
}

/// System: обработка ProjectileHit событий → нанесение урона
/// Godot отправляет событие после collision detection
pub fn process_projectile_hits(
    mut hit_events: EventReader<ProjectileHit>,
    mut targets: Query<&mut crate::Health>,
    mut damage_events: EventWriter<crate::combat::DamageDealt>,
) {
    for hit in hit_events.read() {
        crate::log(&format!(
            "🎯 ProjectileHit: shooter={:?} → target={:?} dmg={}",
            hit.shooter, hit.target, hit.damage
        ));

        // Проверка self-hit (не должно быть!)
        if hit.shooter == hit.target {
            crate::log(&format!(
                "⚠️ SELF-HIT DETECTED! Entity {:?} hit itself!",
                hit.shooter
            ));
            continue; // Пропускаем self-damage
        }

        // Наносим урон цели
        if let Ok(mut health) = targets.get_mut(hit.target) {
            let actual_damage = hit.damage.min(health.current);
            health.take_damage(actual_damage);

            // Генерируем DamageDealt event для визуальных эффектов
            damage_events.write(crate::combat::DamageDealt {
                attacker: hit.shooter,
                target: hit.target,
                damage: actual_damage,
                source: crate::combat::DamageSource::Ranged,
            });

            crate::log(&format!(
                "💥 Projectile hit {:?} for {} damage (HP: {} → {})",
                hit.target, actual_damage, health.current + actual_damage, health.current
            ));
        }
    }
}

// ❌ cleanup_projectiles удалена — Godot полностью управляет lifecycle

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projectile_hit_event() {
        let shooter = Entity::PLACEHOLDER;
        let target = Entity::from_raw(1);

        let hit = ProjectileHit {
            shooter,
            target,
            damage: 20,
        };

        assert_eq!(hit.shooter, shooter);
        assert_eq!(hit.damage, 20);
    }
}
