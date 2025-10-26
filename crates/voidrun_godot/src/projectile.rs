//! GodotProjectile — полностью Godot-managed projectile
//!
//! Architecture (ADR-005):
//! - Godot владеет всем lifecycle: spawn, physics, collision, cleanup
//! - ECS получает только ProjectileHit event для damage calculation
//! - Projectile НЕ хранится в ECS (tactical layer only)
//!
//! # Refactored Architecture (Area3D signal-based)
//! - Projectile = Area3D (детектирует shields + bodies)
//! - Signal area_entered → shield collision
//! - Signal body_entered → actor hit
//! - Collision info хранится IN projectile (не в global queue)
//! - GodotProjectileRegistry tracks all projectiles

use godot::prelude::*;
use godot::classes::{Area3D, IArea3D, CharacterBody3D};
use bevy::prelude::Entity;

/// Collision info (хранится в projectile до обработки ECS)
#[derive(Clone, Debug)]
pub struct ProjectileCollisionInfo {
    pub target_instance_id: InstanceId,
    pub impact_point: Vector3,
    pub impact_normal: Vector3,  // Для VFX (spark direction, shield ripple, decals)
}

/// Shield collision info (separate from body collision)
#[derive(Clone, Debug)]
pub struct ProjectileShieldCollisionInfo {
    pub target_entity_id: u64, // Entity ID of shield owner (from StaticBody3D parent metadata)
    pub impact_point: Vector3,
    pub impact_normal: Vector3,  // Для ripple VFX direction
}

/// Projectile — управляется Godot Area3D (signal-based collision)
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct GodotProjectile {
    base: Base<Area3D>,

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

    /// Shield collision info (separate detection via Area3D overlap)
    pub shield_collision_info: Option<ProjectileShieldCollisionInfo>,
}

#[godot_api]
impl IArea3D for GodotProjectile {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            shooter: Entity::PLACEHOLDER,
            direction: Vector3::ZERO,
            speed: 30.0, // Default (переопределяется через setup())
            damage: 15,
            lifetime: 5.0,
            collision_info: None,
            shield_collision_info: None,
        }
    }

    fn ready(&mut self) {
        // Подключаем signals для collision detection
        let callable_area = self.base().callable("on_area_entered");
        self.base_mut().connect("area_entered", &callable_area);

        let callable_body = self.base().callable("on_body_entered");
        self.base_mut().connect("body_entered", &callable_body);
    }

    fn process(&mut self, delta: f64) {
        // 1. Двигаем projectile (простое линейное движение)
        let velocity = self.direction * self.speed * delta as f32;
        let current_pos = self.base().get_global_position();
        self.base_mut().set_global_position(current_pos + velocity);

        // 2. Уменьшаем lifetime
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

    /// Signal handler: Area3D entered (shield collision)
    #[func]
    fn on_area_entered(&mut self, area: Gd<Area3D>) {
        // Проверяем это shield (Layer 16)
        if area.get_collision_layer() & crate::collision_layers::COLLISION_LAYER_SHIELDS == 0 {
            return; // Не shield
        }

        // Получаем entity_id владельца shield (parent Actor node)
        let Some(parent) = area.get_parent() else {
            return;
        };

        if !parent.has_meta("entity_id") {
            return;
        }

        let entity_id_variant = parent.get_meta("entity_id");
        let Ok(entity_id) = entity_id_variant.try_to::<i64>() else {
            return;
        };

        // Проверка self-hit
        if Entity::from_raw(entity_id as u32) == self.shooter {
            return; // Игнорируем свой щит
        }

        // Store shield collision info
        let impact_point = self.base().get_global_position();
        self.shield_collision_info = Some(ProjectileShieldCollisionInfo {
            target_entity_id: entity_id as u64,
            impact_point,
            impact_normal: Vector3::ZERO, // Area3D не имеет normal (TODO: calculate from position)
        });

        voidrun_simulation::log(&format!(
            "🛡️ Projectile hit shield: entity={}, pos={:?}",
            entity_id, impact_point
        ));

        // НЕ удаляем projectile сразу! ECS система обработает collision и удалит позже
    }

    /// Signal handler: Body entered (actor collision)
    #[func]
    fn on_body_entered(&mut self, body: Gd<CharacterBody3D>) {
        let instance_id = body.instance_id();

        // Проверка self-hit через metadata (если есть)
        if body.has_meta("entity_id") {
            let entity_id_variant = body.get_meta("entity_id");
            if let Ok(entity_id) = entity_id_variant.try_to::<i64>() {
                if Entity::from_raw(entity_id as u32) == self.shooter {
                    return; // Игнорируем своё тело
                }
            }
        }

        // Store body collision info
        let impact_point = self.base().get_global_position();
        self.collision_info = Some(ProjectileCollisionInfo {
            target_instance_id: instance_id,
            impact_point,
            impact_normal: Vector3::ZERO, // Area3D не имеет normal
        });

        voidrun_simulation::log(&format!(
            "🎯 Projectile hit body: instance_id={:?}",
            instance_id
        ));

        // НЕ удаляем projectile сразу! ECS система обработает collision и удалит позже
    }
}
