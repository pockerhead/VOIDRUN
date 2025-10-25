//! GodotProjectile — полностью Godot-managed projectile
//!
//! Architecture (ADR-005):
//! - Godot владеет всем lifecycle: spawn, physics, collision, cleanup
//! - ECS получает только ProjectileHit event для damage calculation
//! - Projectile НЕ хранится в ECS (tactical layer only)
//!
//! # Refactored Architecture (event-driven)
//! - Collision info хранится IN projectile (не в global queue)
//! - GodotProjectileRegistry tracks all projectiles
//! - Collision processing через dedicated ECS system

use godot::prelude::*;
use godot::classes::{CharacterBody3D, ICharacterBody3D};
use bevy::prelude::Entity;

/// Collision info (хранится в projectile до обработки ECS)
#[derive(Clone, Debug)]
pub struct ProjectileCollisionInfo {
    pub target_instance_id: InstanceId,
    pub impact_point: Vector3,
    pub impact_normal: Vector3,  // Для VFX (spark direction, shield ripple, decals)
}

/// Projectile — управляется Godot physics
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct GodotProjectile {
    base: Base<CharacterBody3D>,

    /// Кто выстрелил (для предотвращения self-hit + damage attribution)
    pub shooter: Entity,

    /// Направление полёта
    pub direction: Vector3,

    /// Скорость (м/с)
    pub speed: f32,

    /// Урон
    pub damage: u32,

    /// Время жизни (секунды)
    pub lifetime: f32,

    /// Collision info (хранится в projectile, обрабатывается ECS системой)
    pub collision_info: Option<ProjectileCollisionInfo>,
}

#[godot_api]
impl ICharacterBody3D for GodotProjectile {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            shooter: Entity::PLACEHOLDER,
            direction: Vector3::ZERO,
            speed: 30.0, // Default (переопределяется через setup())
            damage: 15,
            lifetime: 5.0,
            collision_info: None,
        }
    }

    fn physics_process(&mut self, delta: f64) {
        // Debug: проверяем что physics_process вызывается
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
            if FRAME_COUNT % 60 == 0 {
                voidrun_simulation::log(&format!(
                    "Projectile physics_process running (lifetime={:.1}s)",
                    self.lifetime
                ));
            }
        }

        // 1. Двигаем projectile
        let velocity = self.direction * self.speed;
        self.base_mut().set_velocity(velocity);

        let collision = self.base_mut().move_and_collide(velocity * delta as f32);

        // Debug: логируем что вернул move_and_collide
        static mut COLLISION_CHECK_COUNT: u32 = 0;
        unsafe {
            COLLISION_CHECK_COUNT += 1;
            if COLLISION_CHECK_COUNT % 60 == 0 {
                let has_collision = collision.is_some();
                voidrun_simulation::log(&format!(
                    "move_and_collide returned: has_collision={}",
                    has_collision
                ));
            }
        }

        // 2. Store collision info (НЕ пушим в queue!)
        if let Some(godot_collision) = collision {
            if let Some(collider_node) = godot_collision.get_collider() {
                let instance_id = collider_node.instance_id();
                let normal = godot_collision.get_normal();

                // ✅ Store collision info IN projectile
                self.collision_info = Some(ProjectileCollisionInfo {
                    target_instance_id: instance_id,
                    impact_point: self.base().get_global_position(),
                    impact_normal: normal,
                });

                voidrun_simulation::log(&format!(
                    "🎯 Projectile stored collision: instance_id={:?}, normal={:?}",
                    instance_id, normal
                ));

                // NOTE: Projectile НЕ удаляется здесь!
                // ECS система projectile_collision_system_main_thread удалит после обработки.
            }
        }

        // 3. Уменьшаем lifetime
        self.lifetime -= delta as f32;

        if self.lifetime <= 0.0 {
            // Удаляем projectile по истечению времени
            self.base_mut().queue_free();
        }
    }
}

#[godot_api]
impl GodotProjectile {
    /// Установить параметры projectile при spawn
    #[func]
    pub fn setup(&mut self, shooter_raw: i64, direction: Vector3, speed: f32, damage: i64) {
        self.shooter = Entity::from_raw(shooter_raw as u32);
        self.direction = direction.normalized();
        self.speed = speed;
        self.damage = damage as u32;

        voidrun_simulation::log(&format!(
            "Projectile setup: shooter={:?} dir={:?} speed={} dmg={}",
            self.shooter, self.direction, self.speed, self.damage
        ));
    }
}
