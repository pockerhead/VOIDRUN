//! Damage calculation and death systems.

use bevy::prelude::*;
use crate::components::{Health, Stamina};
use crate::combat::{WeaponStats, DamageDealt, EntityDied, DamageSource, AppliedDamage};

/// Компонент-маркер: entity мертв (Health <= 0)
///
/// Используется для визуальных эффектов (death animation, fade-out).
/// Деспавн не автоматический — трупы остаются на месте.
#[derive(Component, Debug)]
pub struct Dead;

/// Компонент-маркер: деспавн entity после указанного времени
///
/// Используется для автоматической уборки мёртвых акторов.
/// Система `despawn_after_timeout` проверяет время и удаляет entity + Godot node.
#[derive(Component, Debug)]
pub struct DespawnAfter {
    /// Время деспавна (в секундах от старта игры)
    pub despawn_time: f32,
}

/// Система: apply damage (placeholder для Godot-driven combat)
///
/// TODO: Будет читать GodotCombatEvent::WeaponHit когда Godot integration готов
/// Сейчас: stub system для компиляции
pub fn apply_damage(
    mut _damage_dealt_events: EventWriter<DamageDealt>,
    mut _entity_died_events: EventWriter<EntityDied>,
    mut _targets: Query<(&mut Health, Option<&Stamina>)>,
    _attackers: Query<(&WeaponStats, &Stamina)>,
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

/// Apply damage with shield absorption logic
///
/// Shield blocks ONLY Ranged damage (slow kinetic like melee bypasses shield).
/// Returns AppliedDamage for VFX feedback.
///
/// # Logic
/// - Ranged damage: Shield absorbs if active, overflow goes to health
/// - Melee damage: Bypasses shield completely (slow kinetic)
/// - Environmental: Direct damage (TODO: future logic)
pub fn apply_damage_with_shield(
    target_health: &mut crate::Health,
    target_shield: Option<&mut crate::components::EnergyShield>,
    damage: u32,
    damage_source: DamageSource,
) -> AppliedDamage {
    // Shield blocks ONLY Ranged (and only if active)
    // When shield is inactive (current_energy <= 0 OR not reached 50% threshold),
    // projectile passes through and hits body directly
    if damage_source == DamageSource::Ranged {
        if let Some(shield) = target_shield {
            // Check if shield is active (hysteresis: deactivates at 0%, reactivates at 50%)
            if shield.is_active() {
                let shield_damage = damage as f32;
                shield.take_damage(shield_damage);
                shield.update_active_state(); // Update active state after damage

                // Shield broke? → overflow damage to health
                if shield.current_energy <= 0.0 {
                    let overflow = (-shield.current_energy) as u32;
                    if overflow > 0 {
                        target_health.take_damage(overflow);
                        crate::logger::log(&format!(
                            "💥 Shield BROKEN! Overflow: {} damage",
                            overflow
                        ));
                        return AppliedDamage::ShieldBrokenWithOverflow(overflow);
                    }
                }

                crate::logger::log("🛡️ Shield absorbed damage");
                return AppliedDamage::ShieldAbsorbed;
            } else {
                // Shield exists but inactive → direct damage to body
                crate::logger::log("🛡️ Shield INACTIVE — projectile bypassed shield");
            }
        }
    }

    // Melee, Environmental, или щита нет → прямой урон
    target_health.take_damage(damage);
    AppliedDamage::Direct
}

/// System: Shield recharge (вне боя) + hysteresis update
///
/// Tick shield energy regeneration после recharge_delay.
/// Updates active state based on hysteresis logic (deactivate at 0%, reactivate at 50%).
/// Runs in FixedUpdate (64 Hz).
pub fn shield_recharge_system(
    mut shields: Query<&mut crate::components::EnergyShield>,
    time: Res<Time>,
) {
    for mut shield in shields.iter_mut() {
        shield.tick(time.delta_secs());
        shield.update_active_state(); // Hysteresis logic (activate at 50%)
    }
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

            crate::logger::log(&format!("INFO: Disabled AI for dead entity {:?}", event.entity));
        }
    }
}

/// Система: деспавн entities с истёкшим DespawnAfter timeout
///
/// Проверяет все entities с компонентом DespawnAfter.
/// Удаляет entity если текущее время >= despawn_time.
/// Godot node удаляется автоматически в despawn_actor_visuals_main_thread.
pub fn despawn_after_timeout(
    mut commands: Commands,
    query: Query<(Entity, &DespawnAfter)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs();

    for (entity, despawn_after) in query.iter() {
        if current_time >= despawn_after.despawn_time {
            crate::logger::log(&format!("⚰️ Despawning entity {:?} (timeout)", entity));
            commands.entity(entity).despawn();
        }
    }
}
