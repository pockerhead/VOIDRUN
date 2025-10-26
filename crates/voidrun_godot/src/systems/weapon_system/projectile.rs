//! Projectile collision detection and melee telegraph systems.

use bevy::prelude::*;
use godot::prelude::*;
use voidrun_simulation::*;
use voidrun_simulation::combat::{AttackType, MeleeAttackState, WeaponStats};
use voidrun_simulation::ai::{GodotAIEvent, SpottedEnemies};
use crate::systems::VisualRegistry;
use crate::actor_utils::{actors_facing_each_other, angles};

// ============================================================================
// Systems: Projectile Collision Detection
// ============================================================================

/// System: Process projectile collisions (Godot → ECS)
///
/// Reads collision info from GodotProjectile nodes.
/// Generates ProjectileHit events для ECS damage processing.
/// Despawns projectiles after processing.
///
/// **Frequency:** Every frame (60 Hz)
pub fn projectile_collision_system_main_thread(
    mut registry: NonSendMut<crate::projectile_registry::GodotProjectileRegistry>,
    visuals: NonSend<VisualRegistry>,
    mut projectile_hit_events: EventWriter<voidrun_simulation::combat::ProjectileHit>,
) {
    // Cleanup destroyed projectiles first
    registry.cleanup_destroyed();

    // Process collisions
    let mut to_remove = Vec::new();

    for (instance_id, mut projectile) in registry.projectiles.iter_mut() {
        // Check if projectile has collision info
        let Some(collision_info) = projectile.bind().collision_info.clone() else {
            continue;  // No collision yet
        };

        // Reverse lookup: InstanceId → Entity
        let Some(&target_entity) = visuals.node_to_entity.get(&collision_info.target_instance_id) else {
            voidrun_simulation::log(&format!(
                "⚠️ Projectile collision with unknown entity (InstanceId: {:?})",
                collision_info.target_instance_id
            ));
            to_remove.push(*instance_id);
            projectile.queue_free();
            continue;
        };

        // Check self-hit (projectile не должна попадать в shooter)
        let shooter = projectile.bind().shooter;
        if target_entity == shooter {
            voidrun_simulation::log(&format!(
                "🚫 Projectile ignored self-collision: shooter={:?}",
                shooter
            ));
            // Clear collision info, projectile продолжает лететь
            projectile.bind_mut().collision_info = None;
            continue;
        };

        // ✅ Generate ProjectileHit event (Godot → ECS) with impact data
        let damage = projectile.bind().damage;
        let impact_point = bevy::prelude::Vec3::new(
            collision_info.impact_point.x,
            collision_info.impact_point.y,
            collision_info.impact_point.z,
        );
        let impact_normal = bevy::prelude::Vec3::new(
            collision_info.impact_normal.x,
            collision_info.impact_normal.y,
            collision_info.impact_normal.z,
        );

        projectile_hit_events.write(voidrun_simulation::combat::ProjectileHit {
            shooter,
            target: target_entity,
            damage,
            impact_point,
            impact_normal,
        });

        voidrun_simulation::log(&format!(
            "💥 Projectile hit! Shooter: {:?} → Target: {:?}, Damage: {} at {:?} (normal: {:?})",
            shooter, target_entity, damage, impact_point, impact_normal
        ));

        // Despawn projectile
        to_remove.push(*instance_id);
        projectile.queue_free();
    }

    // Cleanup processed projectiles from registry
    for instance_id in to_remove {
        registry.unregister(instance_id);
    }
}

/// System: Projectile → Shield collision detection (Godot tactical layer)
///
/// Architecture (Hybrid approach):
/// - Godot: ShieldSphere (Area3D) collision detection → generate ProjectileShieldHit events
/// - ECS: Shield damage + energy depletion (process_projectile_shield_hits in weapon.rs)
/// - Fallback: Point-blank shots bypass ShieldSphere but ECS still blocks via DamageSource::Ranged
///
/// **Self-shield bypass:** shooter == target check (own projectiles don't hit own shield)
/// **Depleted shield bypass:** energy <= 0 → projectile passes through (checked in ECS)
/// **VFX feedback:** Ripple effect on shield mesh (shader uniforms updated in shield_vfx_system.rs)
pub fn projectile_shield_collision_main_thread(
    mut registry: NonSendMut<crate::projectile_registry::GodotProjectileRegistry>,
    visuals: NonSend<VisualRegistry>,
    shields: Query<(Entity, &Actor, &components::EnergyShield)>,
    mut projectile_shield_hit_events: EventWriter<voidrun_simulation::combat::ProjectileShieldHit>,
) {
    let mut to_remove = Vec::new();

    for (&instance_id, projectile) in registry.projectiles.iter_mut() {
        // Check if projectile has collided with ShieldSphere (Area3D)
        let collision_info = projectile.bind().shield_collision_info.clone();
        let Some(collision_info) = collision_info else {
            continue;
        };

        // Get target entity from collision
        let target_entity_id = collision_info.target_entity_id;
        let Some((target_entity, target_actor, target_shield)) = shields
            .iter()
            .find(|(entity, _, _)| entity.to_bits() == target_entity_id)
        else {
            voidrun_simulation::log(&format!(
                "⚠️ Shield collision entity {:?} not found in ECS (already dead?)",
                target_entity_id
            ));
            projectile.bind_mut().shield_collision_info = None;
            continue;
        };

        // ✅ CRITICAL: Self-shield bypass (own projectiles don't hit own shield)
        let shooter = projectile.bind().shooter;
        if target_entity == shooter {
            voidrun_simulation::log(&format!(
                "🛡️ Self-shield bypass: shooter={:?} (projectile passes through own shield)",
                shooter
            ));
            // Clear shield collision, projectile continues (may hit body or other shields)
            projectile.bind_mut().shield_collision_info = None;
            continue;
        }

        // ✅ Depleted shield bypass: energy <= 0 → projectile continues through
        if target_shield.current_energy <= 0.0 {
            voidrun_simulation::log(&format!(
                "🛡️ Depleted shield bypass: target={:?} (0 energy, projectile passes through)",
                target_entity
            ));
            projectile.bind_mut().shield_collision_info = None;
            continue;
        }

        // ✅ Generate ProjectileShieldHit event (Godot → ECS)
        let damage = projectile.bind().damage;
        let impact_point = bevy::prelude::Vec3::new(
            collision_info.impact_point.x,
            collision_info.impact_point.y,
            collision_info.impact_point.z,
        );
        let impact_normal = bevy::prelude::Vec3::new(
            collision_info.impact_normal.x,
            collision_info.impact_normal.y,
            collision_info.impact_normal.z,
        );

        projectile_shield_hit_events.write(voidrun_simulation::combat::ProjectileShieldHit {
            projectile: Entity::PLACEHOLDER, // Projectile despawn handled here, not in ECS
            shooter,
            target: target_entity,
            damage,
            impact_point,
            impact_normal,
        });

        voidrun_simulation::log(&format!(
            "🛡️ Shield hit! Shooter: {:?} → Shield: {:?}, Damage: {} at {:?} (energy: {}/{})",
            shooter, target_entity, damage, impact_point, target_shield.current_energy, target_shield.max_energy
        ));

        // Despawn projectile (shield stopped it)
        to_remove.push(instance_id);
        projectile.queue_free();
    }

    // Cleanup processed projectiles from registry
    for instance_id in to_remove {
        registry.unregister(instance_id);
    }
}

// ============================================================================
// Systems: Melee Windup Detection (Tactical Layer)
// ============================================================================

/// System: Detect visible melee windups (CombatUpdate, 10 Hz)
///
/// For all actors in Windup phase:
/// - Spatial query: enemies within weapon range
/// - Angle check: **MUTUAL FACING** (both attacker→defender AND defender→attacker within 35° cone)
/// - Visibility: defender in attacker's SpottedEnemies
/// - Emit: GodotAIEvent::EnemyWindupVisible (broadcast to all visible defenders)
///
/// **AI реагирует на визуальные cues (реалистично, работает для player + AI)**
///
/// **Frequency:** 10 Hz (CombatUpdate schedule)
/// **Parameters:** Hardcoded (angle 35°, будущий балансинг через WeaponStats)
pub fn detect_melee_windups_main_thread(
    attackers: Query<(Entity, &Actor, &MeleeAttackState, &WeaponStats, &SpottedEnemies)>,
    defenders: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    mut ai_events: EventWriter<GodotAIEvent>,
) {
    for (attacker_entity, attacker_actor, attack_state, weapon, spotted) in attackers.iter() {
        // Только Windup phase
        if !attack_state.is_windup() {
            continue;
        }

        // Godot Transform (tactical layer)
        let Some(attacker_node) = visuals.visuals.get(&attacker_entity) else {
            continue;
        };

        let attacker_pos = attacker_node.get_global_position();

        // Spatial query: все видимые враги в spotted
        for &defender_entity in &spotted.enemies {
            // Проверка faction (только враги)
            let Ok(defender_actor) = defenders.get(defender_entity) else {
                continue;
            };

            if defender_actor.faction_id == attacker_actor.faction_id {
                continue;
            }

            // Distance check
            let Some(defender_node) = visuals.visuals.get(&defender_entity) else {
                continue;
            };

            let defender_pos = defender_node.get_global_position();
            let distance = (defender_pos - attacker_pos).length();

            if distance > weapon.attack_radius {
                continue;
            }

            // ✅ MUTUAL FACING CHECK (using actor_utils)
            let Some((dot_attacker, dot_defender)) = actors_facing_each_other(
                attacker_node,
                defender_node,
                angles::TIGHT_35_DEG,
            ) else {
                continue; // Not facing each other
            };

            // ✅ MUTUAL FACING - DEFENDER CAN SEE WINDUP!
            ai_events.write(GodotAIEvent::EnemyWindupVisible {
                attacker: attacker_entity,
                defender: defender_entity,
                attack_type: AttackType::Melee, // Всегда Melee для melee атак
                windup_remaining: attack_state.phase_timer,
            });

            voidrun_simulation::log(&format!(
                "👁️ Windup visible (MUTUAL FACING): {:?} → {:?} (distance: {:.1}m, attacker_angle: {:.2}, defender_angle: {:.2}, windup: {:.2}s)",
                attacker_entity, defender_entity, distance, dot_attacker, dot_defender, attack_state.phase_timer
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_aim_only_in_combat() {
        // Verify aim system only triggers in Combat state
        // (unit test без Godot API)
    }
}
