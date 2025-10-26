//! Shield VFX System
//!
//! Обновляет shader uniforms для ShieldMesh на основе EnergyShield состояния.
//!
//! # Systems
//! - `update_shield_energy_vfx_main_thread()` — обновляет `energy_percent` uniform
//! - `update_shield_ripple_vfx_main_thread()` — обновляет `last_hit_pos` и `last_hit_time` uniforms
//!
//! # Architecture
//! - Runs in MainThreadUpdate (Godot API calls)
//! - Query: `Changed<EnergyShield>` (reactive — только когда энергия меняется)
//! - Events: `ProjectileShieldHit` (для ripple VFX)
//! - Uniforms: `energy_percent`, `last_hit_pos`, `last_hit_time`

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{MeshInstance3D, ShaderMaterial, StaticBody3D};
use voidrun_simulation::logger;

use voidrun_simulation::shared::equipment::EnergyShield;
use crate::shared::VisualRegistry;
use crate::collision_layers::COLLISION_LAYER_SHIELDS;

/// System: Update shield shader uniforms on SIGNIFICANT energy change
///
/// Listens to `Changed<EnergyShield>` и обновляет `energy_percent` uniform
/// в ShaderMaterial ТОЛЬКО при изменении >5% энергии.
///
/// **Note:** Changed<EnergyShield> триггерится каждый frame из-за tick() в recharge system.
/// Мы фильтруем обновления чтобы избежать spam shader updates.
///
/// # Flow
/// 1. Query actors с Changed<EnergyShield>
/// 2. Calculate energy_percent
/// 3. Get CURRENT shader uniform value
/// 4. Update ONLY if delta > 5%
///
/// # Runs
/// MainThreadUpdate (Godot API access)
pub fn update_shield_energy_vfx_main_thread(
    shields: Query<(Entity, &EnergyShield), Changed<EnergyShield>>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, shield) in shields.iter() {
        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Get ShieldSphere node
        let Some(shield_sphere) = actor_node.try_get_node_as::<Node3D>("ShieldSphere") else {
            continue;
        };

        // Get ShieldMesh
        let Some(mut shield_mesh) = shield_sphere.try_get_node_as::<MeshInstance3D>("ShieldMesh") else {
            continue;
        };

        // Get ShaderMaterial (surface_material_override/0)
        let Some(material) = shield_mesh.get_surface_override_material(0) else {
            continue;
        };

        let mut shader_mat = material.cast::<ShaderMaterial>();

        // Calculate NEW energy_percent (0.0-1.0)
        let new_energy_percent = (shield.current_energy / shield.max_energy).clamp(0.0, 1.0);

        // Get CURRENT shader uniform value
        let current_variant = shader_mat.get_shader_parameter("energy_percent");
        let current_energy_percent = current_variant.try_to::<f32>().unwrap_or(1.0);

        // Calculate delta (absolute difference)
        let delta = (new_energy_percent - current_energy_percent).abs();

        // Update ONLY if delta > 5% (threshold to avoid spam)
        const THRESHOLD: f32 = 0.05;
        if delta > THRESHOLD {
            let energy_variant = Variant::from(new_energy_percent);
            shader_mat.set_shader_parameter("energy_percent", &energy_variant);

            logger::log(&format!(
                "🛡️ Shield VFX updated: entity={:?}, energy={:.0}/{:.0} ({:.0}% → {:.0}%)",
                entity,
                shield.current_energy,
                shield.max_energy,
                current_energy_percent * 100.0,
                new_energy_percent * 100.0
            ));
        }
    }
}

/// System: Update shield collision layer based on active state
///
/// Включает/выключает collision layer щита на основе `is_active` состояния.
/// - Active shield (is_active = true): collision_layer = 16 (SHIELDS)
/// - Inactive shield (is_active = false): collision_layer = 0 (no collision)
///
/// # Flow
/// 1. Query entities с Changed<EnergyShield>
/// 2. Get ShieldSphere StaticBody3D node
/// 3. Set collision_layer based on is_active state
///
/// # Runs
/// MainThreadUpdate (Godot API access)
pub fn update_shield_collision_state_main_thread(
    shields: Query<(Entity, &EnergyShield), Changed<EnergyShield>>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, shield) in shields.iter() {
        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Get ShieldSphere StaticBody3D node
        let Some(mut shield_sphere) = actor_node.try_get_node_as::<StaticBody3D>("ShieldSphere") else {
            continue;
        };

        // Set collision layer based on active state
        let collision_layer = if shield.is_active() {
            COLLISION_LAYER_SHIELDS // 16 — projectiles detect shield
        } else {
            0 // No collision — projectiles pass through
        };

        shield_sphere.set_collision_layer(collision_layer);

        logger::log(&format!(
            "🛡️ Shield collision state updated: entity={:?}, is_active={}, collision_layer={}",
            entity, shield.is_active(), collision_layer
        ));
    }
}

/// System: Update shield ripple VFX on projectile hit
///
/// Listens to `ProjectileShieldHit` events и обновляет ripple shader uniforms:
/// - `last_hit_pos` (Vec3) — точка попадания для ripple epicenter
/// - `last_hit_time` (f32) — время попадания (Bevy Time elapsed)
///
/// Shader использует эти данные для создания expanding ripple wave.
///
/// # Flow
/// 1. Read ProjectileShieldHit events
/// 2. Get target actor visual node
/// 3. Get ShieldSphere/ShieldMesh/ShaderMaterial
/// 4. Update `last_hit_pos` и `last_hit_time` uniforms
///
/// # Runs
/// MainThreadUpdate (Godot API access)
pub fn update_shield_ripple_vfx_main_thread(
    mut hit_events: EventReader<voidrun_simulation::combat::ProjectileShieldHit>,
    visuals: NonSend<VisualRegistry>,
    time: Res<Time>,
) {
    for hit in hit_events.read() {
        let Some(actor_node) = visuals.visuals.get(&hit.target) else {
            continue;
        };

        // Get ShieldSphere node
        let Some(shield_sphere) = actor_node.try_get_node_as::<Node3D>("ShieldSphere") else {
            continue;
        };

        // Get ShieldMesh
        let Some(mut shield_mesh) = shield_sphere.try_get_node_as::<MeshInstance3D>("ShieldMesh") else {
            continue;
        };

        // Get ShaderMaterial (surface_material_override/0)
        let Some(material) = shield_mesh.get_surface_override_material(0) else {
            continue;
        };

        let mut shader_mat = material.cast::<ShaderMaterial>();

        // Convert impact_point to Godot Vector3
        let impact_pos = Vector3::new(
            hit.impact_point.x,
            hit.impact_point.y,
            hit.impact_point.z,
        );

        // Get current time in seconds (for ripple animation)
        let current_time = time.elapsed_secs();

        // Update shader uniforms
        let pos_variant = Variant::from(impact_pos);
        let time_variant = Variant::from(current_time);

        shader_mat.set_shader_parameter("last_hit_pos", &pos_variant);
        shader_mat.set_shader_parameter("last_hit_time", &time_variant);

        logger::log(&format!(
            "🌊 Shield ripple VFX: target={:?}, pos={:?}, time={:.2}s",
            hit.target, impact_pos, current_time
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_percent_calculation() {
        let shield = EnergyShield {
            max_energy: 200.0,
            current_energy: 100.0,
            ..Default::default()
        };

        let energy_percent = (shield.current_energy / shield.max_energy).clamp(0.0, 1.0);
        assert_eq!(energy_percent, 0.5);
    }

    #[test]
    fn test_energy_percent_clamp() {
        let shield = EnergyShield {
            max_energy: 100.0,
            current_energy: -10.0, // Overflow случай
            ..Default::default()
        };

        let energy_percent = (shield.current_energy / shield.max_energy).clamp(0.0, 1.0);
        assert_eq!(energy_percent, 0.0); // Clamp к 0.0
    }
}
