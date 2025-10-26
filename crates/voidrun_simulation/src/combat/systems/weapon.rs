//! Weapon systems (cooldowns + ranged combat).

use bevy::prelude::*;
use crate::combat::{
    WeaponStats, WeaponFireIntent, ProjectileHit, ProjectileShieldHit, DamageDealt, DamageSource,
};

/// System: обновление weapon cooldowns
pub fn update_weapon_cooldowns(
    mut weapons: Query<&mut WeaponStats>,
    time: Res<Time>,
) {
    for mut weapon in weapons.iter_mut() {
        if weapon.cooldown_timer > 0.0 {
            weapon.cooldown_timer -= time.delta_secs();
            weapon.cooldown_timer = weapon.cooldown_timer.max(0.0);
        }
    }
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
    mut actors: Query<(Entity, &crate::ai::AIState, &mut WeaponStats)>,
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

        crate::logger::log(&format!(
            "Actor {:?} wants to fire at {:?} (intent generated)",
            entity, target
        ));
    }
}

/// System: обработка ProjectileHit событий → нанесение урона
///
/// Godot отправляет событие после collision detection.
/// Применяет damage с учётом shield (ranged блокируется щитом).
pub fn process_projectile_hits(
    mut hit_events: EventReader<ProjectileHit>,
    mut targets: Query<(&mut crate::Health, Option<&mut crate::components::EnergyShield>)>,
    mut damage_events: EventWriter<DamageDealt>,
) {
    for hit in hit_events.read() {
        crate::logger::log(&format!(
            "🎯 ProjectileHit: shooter={:?} → target={:?} dmg={} at {:?}",
            hit.shooter, hit.target, hit.damage, hit.impact_point
        ));

        // Проверка self-hit (не должно быть!)
        if hit.shooter == hit.target {
            crate::logger::log(&format!(
                "⚠️ SELF-HIT DETECTED! Entity {:?} hit itself!",
                hit.shooter
            ));
            continue; // Пропускаем self-damage
        }

        // Наносим урон цели (с учётом shield)
        let Ok((mut health, mut shield_opt)) = targets.get_mut(hit.target) else {
            continue;
        };

        let applied = crate::combat::apply_damage_with_shield(
            &mut health,
            shield_opt.as_deref_mut(),
            hit.damage,
            DamageSource::Ranged,
        );

        // Генерируем DamageDealt event для визуальных эффектов
        damage_events.write(DamageDealt {
            attacker: hit.shooter,
            target: hit.target,
            damage: hit.damage,
            source: DamageSource::Ranged,
            applied_damage: applied,
            impact_point: hit.impact_point,
            impact_normal: hit.impact_normal,
        });

        crate::logger::log(&format!(
            "💥 Projectile damage applied: {:?} (HP: {})",
            applied, health.current
        ));
    }
}

/// System: обработка ProjectileShieldHit событий → разрядка щита
///
/// Godot отправляет событие когда projectile коллидирует с ShieldSphere.
/// Применяет damage только к щиту (урон в health не проходит).
/// Self-shield bypass уже проверен в Godot layer.
pub fn process_projectile_shield_hits(
    mut hit_events: EventReader<ProjectileShieldHit>,
    mut targets: Query<(&mut crate::Health, Option<&mut crate::components::EnergyShield>)>,
    mut damage_events: EventWriter<DamageDealt>,
) {
    for hit in hit_events.read() {
        crate::logger::log(&format!(
            "🛡️ ProjectileShieldHit: shooter={:?} → shield={:?} dmg={} at {:?}",
            hit.shooter, hit.target, hit.damage, hit.impact_point
        ));

        // Paranoid validation: shooter != target (должно быть уже проверено в Godot)
        if hit.shooter == hit.target {
            crate::logger::log(&format!(
                "⚠️ SELF-SHIELD HIT! This should never happen (Godot bug?). Entity {:?}",
                hit.shooter
            ));
            continue;
        }

        // Наносим урон щиту (не трогаем health)
        let Ok((mut health, mut shield_opt)) = targets.get_mut(hit.target) else {
            continue;
        };

        let applied = crate::combat::apply_damage_with_shield(
            &mut health,
            shield_opt.as_deref_mut(),
            hit.damage,
            DamageSource::Ranged, // Shield blocks ranged
        );

        // Генерируем DamageDealt event для визуальных эффектов
        damage_events.write(DamageDealt {
            attacker: hit.shooter,
            target: hit.target,
            damage: hit.damage,
            source: DamageSource::Ranged,
            applied_damage: applied,
            impact_point: hit.impact_point,
            impact_normal: hit.impact_normal,
        });

        crate::logger::log(&format!(
            "🛡️ Shield absorbed damage: {:?} (HP: {} — untouched)",
            applied, health.current
        ));
    }
}
