//! GodotProjectileRegistry — track Godot-managed projectiles for collision processing
//!
//! # Architecture
//! - Хранит ссылки на GodotProjectile nodes для collision processing
//! - Обновляется каждый frame (добавляем new projectiles, удаляем destroyed)
//! - ECS система читает collision_info из projectiles → генерирует events

use godot::prelude::*;
use std::collections::HashMap;
use crate::projectile::GodotProjectile;
use voidrun_simulation::logger;

/// Registry для Godot projectiles
///
/// Хранит ссылки на GodotProjectile nodes для collision processing.
/// Обновляется каждый frame (добавляем new projectiles, удаляем destroyed).
#[derive(Default)]
pub struct GodotProjectileRegistry {
    /// InstanceId → GodotProjectile node
    pub projectiles: HashMap<InstanceId, Gd<GodotProjectile>>,
}

impl GodotProjectileRegistry {
    /// Register new projectile
    pub fn register(&mut self, projectile: Gd<GodotProjectile>) {
        let instance_id = projectile.instance_id();
        self.projectiles.insert(instance_id, projectile);
        logger::log(&format!("📋 Registered projectile: {:?}", instance_id));
    }

    /// Unregister projectile (after despawn)
    pub fn unregister(&mut self, instance_id: InstanceId) {
        self.projectiles.remove(&instance_id);
        logger::log(&format!("🗑️ Unregistered projectile: {:?}", instance_id));
    }

    /// Cleanup destroyed projectiles (call every frame)
    ///
    /// Removes projectiles that were queue_free()'d by Godot.
    pub fn cleanup_destroyed(&mut self) {
        self.projectiles.retain(|id, proj| {
            let is_valid = proj.is_instance_valid();
            if !is_valid {
                logger::log(&format!("🗑️ Cleanup destroyed projectile: {:?}", id));
            }
            is_valid
        });
    }
}
