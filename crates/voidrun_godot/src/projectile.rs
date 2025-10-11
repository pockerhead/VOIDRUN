//! GodotProjectile — полностью Godot-managed projectile
//!
//! Architecture (ADR-005):
//! - Godot владеет всем lifecycle: spawn, physics, collision, cleanup
//! - ECS получает только ProjectileHit event для damage calculation
//! - Projectile НЕ хранится в ECS (tactical layer only)

use godot::prelude::*;
use godot::classes::{CharacterBody3D, ICharacterBody3D};
use bevy::prelude::Entity;
use voidrun_simulation::combat::ProjectileHit;
use std::collections::HashMap;

/// Static queue для ProjectileHit events (Godot → ECS)
/// Godot не имеет прямого доступа к EventWriter, поэтому используем queue
static mut PROJECTILE_HIT_QUEUE: Option<Vec<ProjectileHit>> = None;

/// Static reverse mapping InstanceId → Entity (для projectile collision lookup)
static mut NODE_TO_ENTITY: Option<HashMap<godot::prelude::InstanceId, Entity>> = None;

pub fn init_projectile_hit_queue() {
    unsafe {
        PROJECTILE_HIT_QUEUE = Some(Vec::new());
        NODE_TO_ENTITY = Some(HashMap::new());
    }
}

pub fn register_collision_body(instance_id: godot::prelude::InstanceId, entity: Entity) {
    unsafe {
        if let Some(map) = NODE_TO_ENTITY.as_mut() {
            map.insert(instance_id, entity);
        }
    }
}

pub fn take_projectile_hits() -> Vec<ProjectileHit> {
    unsafe {
        PROJECTILE_HIT_QUEUE.as_mut().map(|q| q.drain(..).collect()).unwrap_or_default()
    }
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

        // 2. Проверяем collision
        if let Some(collision_info) = collision {
            voidrun_simulation::log(&format!(
                "🎯 Projectile collision detected! shooter={:?}",
                self.shooter
            ));

            // Получаем Entity из collider (reverse lookup через InstanceId)
            let collider = collision_info.get_collider();
            if let Some(collider_node) = collider {
                let instance_id = collider_node.instance_id();

                voidrun_simulation::log(&format!(
                    "  Collider: InstanceId={:?}",
                    instance_id
                ));

                // Reverse lookup InstanceId → Entity
                let mut should_destroy = false;

                unsafe {
                    if let Some(map) = NODE_TO_ENTITY.as_ref() {
                        if let Some(&target_entity) = map.get(&instance_id) {
                            // Игнорируем self-hit (projectile не должна попадать в shooter)
                            if target_entity == self.shooter {
                                voidrun_simulation::log(&format!(
                                    "Projectile ignored self-collision: shooter={:?}",
                                    self.shooter
                                ));
                                // НЕ удаляем projectile - продолжает лететь
                            } else {
                                // Отправляем ProjectileHit в queue
                                if let Some(queue) = PROJECTILE_HIT_QUEUE.as_mut() {
                                    queue.push(ProjectileHit {
                                        shooter: self.shooter,
                                        target: target_entity,
                                        damage: self.damage,
                                    });

                                    voidrun_simulation::log(&format!(
                                        "Projectile hit! Shooter: {:?} → Target: {:?}, Damage: {}",
                                        self.shooter, target_entity, self.damage
                                    ));
                                }
                                should_destroy = true; // Удаляем только при реальном попадании
                            }
                        } else {
                            voidrun_simulation::log(&format!(
                                "Projectile collision with unknown entity (InstanceId: {:?})",
                                instance_id
                            ));
                            should_destroy = true; // Удаляем при collision с неизвестным объектом
                        }
                    }
                }

                // Удаляем projectile только если это не self-collision
                if should_destroy {
                    self.base_mut().queue_free();
                    return;
                }
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
