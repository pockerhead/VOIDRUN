//! Hitbox система для combat
//!
//! Архитектура:
//! - AttackHitbox появляется только во время swing (0.2-0.4 sec)
//! - Sphere hitbox (радиус ~1.5m для melee)
//! - Collision detection через Rapier spatial queries
//! - Каждый hitbox живет 1 frame → detect overlap → despawn

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::Health;

/// Hitbox атаки (временный, живет 1 frame)
///
/// Спавнится во время swing animation, проверяет overlap с врагами, despawn.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AttackHitbox {
    /// Радиус сферы hitbox (метры)
    pub radius: f32,
    /// Базовый урон (до модификаторов)
    pub damage: u32,
    /// Кто атакует (для проверки friendly fire)
    pub attacker: Entity,
    /// Время жизни hitbox (секунды), обычно 1 frame
    pub lifetime: f32,
}

impl Default for AttackHitbox {
    fn default() -> Self {
        Self {
            radius: 1.5,
            damage: 0,
            attacker: Entity::PLACEHOLDER,
            lifetime: 0.016,
        }
    }
}

impl AttackHitbox {
    pub fn new(attacker: Entity, damage: u32, radius: f32) -> Self {
        Self {
            radius,
            damage,
            attacker,
            lifetime: 0.016, // 1 frame при 60fps
        }
    }
}

/// Маркер что entity может атаковать
///
/// Содержит параметры атаки (cooldown, damage, range).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Attacker {
    /// Cooldown между атаками (секунды)
    pub attack_cooldown: f32,
    /// Текущий cooldown timer (0 = готов атаковать)
    pub cooldown_timer: f32,
    /// Базовый урон атаки
    pub base_damage: u32,
    /// Радиус атаки (метры)
    pub attack_radius: f32,
}

impl Default for Attacker {
    fn default() -> Self {
        Self {
            attack_cooldown: 1.0, // 1 атака в секунду
            cooldown_timer: 0.0,
            base_damage: 20,
            attack_radius: 1.5, // 1.5m melee range
        }
    }
}

impl Attacker {
    pub fn can_attack(&self) -> bool {
        self.cooldown_timer <= 0.0
    }

    pub fn start_attack(&mut self) {
        self.cooldown_timer = self.attack_cooldown;
    }

    pub fn tick(&mut self, delta: f32) {
        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= delta;
        }
    }
}

/// Событие: атака начата
///
/// Триггерит spawn hitbox.
#[derive(Event, Debug, Clone)]
pub struct AttackStarted {
    pub attacker: Entity,
    pub damage: u32,
    pub radius: f32,
    /// Offset от attacker position (forward direction обычно)
    pub offset: Vec3,
}

/// Событие: hitbox попал по target
#[derive(Event, Debug, Clone)]
pub struct HitboxOverlap {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: u32,
}

/// Система: tick attacker cooldowns
///
/// Уменьшает cooldown_timer для всех Attacker компонентов.
pub fn tick_attack_cooldowns(
    mut attackers: Query<&mut Attacker>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();
    for mut attacker in attackers.iter_mut() {
        attacker.tick(delta);
    }
}

/// Система: spawn hitbox при AttackStarted событии
///
/// Создает временный entity с AttackHitbox + Sensor collider.
pub fn spawn_attack_hitbox(
    mut commands: Commands,
    mut events: EventReader<AttackStarted>,
    attacker_transforms: Query<&Transform>,
) {
    for event in events.read() {
        // Получаем позицию attacker
        let attacker_transform = match attacker_transforms.get(event.attacker) {
            Ok(t) => t,
            Err(_) => {
                crate::log(&format!("WARN: AttackStarted event: attacker entity {:?} not found", event.attacker));
                continue;
            }
        };

        // Spawn hitbox в позиции attacker + offset
        let hitbox_position = attacker_transform.translation + event.offset;

        commands.spawn((
            AttackHitbox::new(event.attacker, event.damage, event.radius),
            Transform::from_translation(hitbox_position),
            GlobalTransform::default(),
            // Rapier sensor (не физическое тело, только detection)
            Collider::ball(event.radius),
            Sensor,
            // Collision groups: атаки попадают только по врагам (настроим позже)
            CollisionGroups::default(),
        ));

        // Debug logging (закомментировано для производительности)
        // crate::log(&format!("DEBUG: Spawned attack hitbox at {:?} (radius: {}, damage: {})",
        //     hitbox_position, event.radius, event.damage));
    }
}

/// Система: detect hitbox overlaps
///
/// Проверяет все AttackHitbox на overlap с Health entities.
/// Генерирует HitboxOverlap события для damage системы.
pub fn detect_hitbox_overlaps(
    mut commands: Commands,
    hitboxes: Query<(Entity, &AttackHitbox, &Transform)>,
    targets: Query<(Entity, &Transform, &Health)>,
    mut overlap_events: EventWriter<HitboxOverlap>,
) {
    for (hitbox_entity, hitbox, hitbox_transform) in hitboxes.iter() {
        let hitbox_pos = hitbox_transform.translation;

        for (target_entity, target_transform, _health) in targets.iter() {
            // Не бьем самого себя
            if target_entity == hitbox.attacker {
                continue;
            }

            let target_pos = target_transform.translation;
            let distance = hitbox_pos.distance(target_pos);

            // Проверяем overlap (простая sphere check)
            // TODO: использовать Rapier intersection queries для точности
            if distance < hitbox.radius {
                overlap_events.write(HitboxOverlap {
                    attacker: hitbox.attacker,
                    target: target_entity,
                    damage: hitbox.damage,
                });

                // Debug logging (закомментировано для производительности)
                // eprintln!("DEBUG: Hitbox overlap: attacker {:?} hit target {:?} (damage: {})",
                //     hitbox.attacker, target_entity, hitbox.damage);
            }
        }

        // Despawn hitbox после проверки (live 1 frame)
        commands.entity(hitbox_entity).despawn();
    }
}

/// Система: tick hitbox lifetime (альтернатива despawn в detect)
///
/// Если хотим чтобы hitbox жил несколько frames — используем эту систему.
/// Пока закомментировано, используем immediate despawn в detect_hitbox_overlaps.
#[allow(dead_code)]
pub fn tick_hitbox_lifetime(
    mut commands: Commands,
    mut hitboxes: Query<(Entity, &mut AttackHitbox)>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (entity, mut hitbox) in hitboxes.iter_mut() {
        hitbox.lifetime -= delta;
        if hitbox.lifetime <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attacker_cooldown() {
        let mut attacker = Attacker::default();

        assert!(attacker.can_attack());

        attacker.start_attack();
        assert!(!attacker.can_attack());
        assert_eq!(attacker.cooldown_timer, 1.0);

        attacker.tick(0.5);
        assert!(!attacker.can_attack());
        assert_eq!(attacker.cooldown_timer, 0.5);

        attacker.tick(0.5);
        assert!(attacker.can_attack());
        assert_eq!(attacker.cooldown_timer, 0.0);
    }

    #[test]
    fn test_hitbox_overlap_detection() {
        // Простой тест логики overlap
        let hitbox_pos = Vec3::new(0.0, 0.0, 0.0);
        let hitbox_radius = 1.5;

        let target_near = Vec3::new(1.0, 0.0, 0.0); // distance = 1.0 < 1.5 ✓
        let target_far = Vec3::new(2.0, 0.0, 0.0);  // distance = 2.0 > 1.5 ✗

        assert!(hitbox_pos.distance(target_near) < hitbox_radius);
        assert!(hitbox_pos.distance(target_far) > hitbox_radius);
    }

    #[test]
    fn test_attack_started_event() {
        let event = AttackStarted {
            attacker: Entity::PLACEHOLDER,
            damage: 25,
            radius: 1.5,
            offset: Vec3::new(0.0, 0.0, 1.0), // 1m вперед
        };

        assert_eq!(event.damage, 25);
        assert_eq!(event.radius, 1.5);
    }
}
