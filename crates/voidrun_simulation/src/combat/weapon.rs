//! Weapon hitbox system
//!
//! Меч-капсула как child entity, активируется во время атаки.
//! Использует rapier collision detection для попадания по врагам.
//!
//! Архитектура:
//! - Weapon компонент хранит состояние (idle/swinging/cooldown)
//! - Spawn weapon как child entity при создании Actor
//! - Attack system активирует swing animation (rotate forward 90°)
//! - Collision detection между weapon и enemy actors

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

/// Collision groups для физики
pub mod collision {
    use bevy_rapier3d::prelude::*;

    /// Actor collision group (персонажи, NPC)
    pub const ACTOR_GROUP: Group = Group::GROUP_1;
    /// Weapon collision group (мечи, хитбоксы атак)
    pub const WEAPON_GROUP: Group = Group::GROUP_2;

    /// Actors НЕ коллайдят друг с другом (проходят насквозь)
    /// Только weapons детектят попадания
    pub fn actor_groups() -> CollisionGroups {
        CollisionGroups::new(ACTOR_GROUP, Group::NONE) // Actors не видят других actors
    }

    /// Weapons коллайдят только с actors, не с другими weapons
    pub fn weapon_groups() -> CollisionGroups {
        CollisionGroups::new(WEAPON_GROUP, ACTOR_GROUP)
    }
}

/// Weapon component (меч)
///
/// Прикреплен к child entity, управляется parent Actor.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Weapon {
    /// Parent actor entity (владелец меча)
    pub owner: Entity,
    /// Damage наносимый этим оружием
    pub damage: u32,
    /// Состояние weapon
    pub state: WeaponState,
    /// Swing timer (для анимации)
    pub swing_timer: f32,
    /// Swing duration (полная длительность взмаха)
    pub swing_duration: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            owner: Entity::PLACEHOLDER,
            damage: 10,
            state: WeaponState::Idle,
            swing_timer: 0.0,
            swing_duration: 0.2, // 200ms swing
        }
    }
}

/// Weapon state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum WeaponState {
    /// Оружие в покое (не активно)
    Idle,
    /// Swing animation в процессе (активный хитбокс)
    Swinging,
    /// Cooldown после атаки (деактивированный хитбокс)
    Cooldown,
}

/// Spawn weapon для actor
///
/// Создает child entity с Weapon компонентом и rapier Collider.
pub fn spawn_weapon(
    commands: &mut Commands,
    owner: Entity,
    damage: u32,
) -> Entity {
    commands
        .spawn((
            // Transform относительно parent (впереди actor, на уровне руки)
            Transform::from_xyz(0.3, 0.3, 1.0) // Вправо 0.3, вверх 0.3, вперёд 1.0
                .with_rotation(Quat::from_euler(
                    bevy::math::EulerRot::XYZ,
                    -30.0_f32.to_radians(), // Наклон вниз 30°
                    0.0,
                    45.0_f32.to_radians(),  // Поворот вправо 45°
                )),

            // Weapon component
            Weapon {
                owner,
                damage,
                ..default()
            },

            // Rapier collider (меч-капсула, длинная 1.5m)
            Collider::capsule_y(0.75, 0.08), // Высота 1.5m (0.75 * 2), радиус 0.08m
            Sensor, // Sensor = не блокирует движение, только detect collisions

            // Collision groups (weapons vs actors)
            collision::weapon_groups(),

            // Active events для collision detection
            ActiveEvents::COLLISION_EVENTS,
        ))
        .id()
}

/// System: Attach weapons к новым actors
///
/// Автоматически создает weapon child entity для каждого Actor.
pub fn attach_weapons_to_actors(
    mut commands: Commands,
    query: Query<Entity, (With<crate::components::Actor>, Without<Children>)>,
) {
    for actor in query.iter() {
        let weapon = spawn_weapon(&mut commands, actor, 10);
        commands.entity(actor).add_child(weapon);
    }
}

/// System: Weapon swing animation
///
/// Обновляет состояние weapon и поворачивает его во время swing.
pub fn weapon_swing_animation(
    mut query: Query<(&mut Weapon, &mut Transform)>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    // Base rotation (diagonal pose: -30° pitch, 45° roll)
    let base_rotation = Quat::from_euler(
        bevy::math::EulerRot::XYZ,
        -30.0_f32.to_radians(),
        0.0,
        45.0_f32.to_radians(),
    );

    for (mut weapon, mut transform) in query.iter_mut() {
        match weapon.state {
            WeaponState::Idle => {
                // Idle: weapon в базовой позиции (диагональ)
                transform.rotation = base_rotation;
            }

            WeaponState::Swinging => {
                // Swinging: удар сверху вниз (pitch -30° → -120°, vertical slash)
                weapon.swing_timer += delta;

                if weapon.swing_timer >= weapon.swing_duration {
                    // Swing завершен → Cooldown
                    weapon.state = WeaponState::Cooldown;
                    weapon.swing_timer = 0.0;
                } else {
                    // Interpolate pitch: -30° → -120° (swing down)
                    let progress = weapon.swing_timer / weapon.swing_duration;
                    let pitch = -30.0 - (progress * 90.0); // -30 → -120
                    transform.rotation = Quat::from_euler(
                        bevy::math::EulerRot::XYZ,
                        pitch.to_radians(),
                        0.0,
                        45.0_f32.to_radians(),
                    );
                }
            }

            WeaponState::Cooldown => {
                // Cooldown: возвращаем weapon в idle позицию
                transform.rotation = base_rotation;

                weapon.swing_timer += delta;
                if weapon.swing_timer >= 0.3 {
                    // Cooldown завершен → Idle
                    weapon.state = WeaponState::Idle;
                    weapon.swing_timer = 0.0;
                }
            }
        }
    }
}

/// System: Trigger weapon swing on attack
///
/// Когда Actor начинает атаку (AttackStarted event) → активируем weapon swing.
pub fn trigger_weapon_swing(
    mut weapon_query: Query<&mut Weapon>,
    actor_query: Query<&Children, With<crate::components::Actor>>,
    mut attack_events: EventReader<crate::combat::AttackStarted>,
) {
    for event in attack_events.read() {
        // Находим weapon child entity для атакующего actor
        if let Ok(children) = actor_query.get(event.attacker) {
            for child in children.iter() {
                if let Ok(mut weapon) = weapon_query.get_mut(child) {
                    // Активируем swing
                    weapon.state = WeaponState::Swinging;
                    weapon.swing_timer = 0.0;
                }
            }
        }
    }
}

/// System: Weapon collision detection
///
/// Обрабатывает столкновения weapon с enemy actors.
/// Генерирует DamageDealt events.
pub fn weapon_collision_detection(
    weapon_query: Query<&Weapon>,
    actor_query: Query<&crate::components::Actor>,
    mut collision_events: EventReader<CollisionEvent>,
    mut damage_events: EventWriter<crate::combat::DamageDealt>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(e1, e2, _flags) = event {
            // Определяем какой entity — weapon, какой — actor
            let (weapon_entity, target_entity) = if weapon_query.contains(*e1) {
                (*e1, *e2)
            } else if weapon_query.contains(*e2) {
                (*e2, *e1)
            } else {
                continue; // Не weapon collision
            };

            // Проверяем что weapon в состоянии Swinging
            if let Ok(weapon) = weapon_query.get(weapon_entity) {
                if weapon.state != WeaponState::Swinging {
                    continue; // Weapon не активен
                }

                // Проверяем что target — Actor (не владелец weapon)
                if let Ok(_target_actor) = actor_query.get(target_entity) {
                    if target_entity == weapon.owner {
                        continue; // Не бьем самого себя
                    }

                    // Генерируем DamageDealt event
                    damage_events.write(crate::combat::DamageDealt {
                        attacker: weapon.owner,
                        target: target_entity,
                        damage: weapon.damage,
                        target_died: false, // Заполнится в apply_damage system
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_default() {
        let weapon = Weapon::default();
        assert_eq!(weapon.damage, 10);
        assert_eq!(weapon.state, WeaponState::Idle);
        assert_eq!(weapon.swing_timer, 0.0);
        assert_eq!(weapon.swing_duration, 0.2);
    }

    #[test]
    fn test_collision_groups() {
        let actor_groups = collision::actor_groups();
        let weapon_groups = collision::weapon_groups();

        // Actors в GROUP_1, weapons в GROUP_2
        assert_eq!(actor_groups.memberships, collision::ACTOR_GROUP);
        assert_eq!(weapon_groups.memberships, collision::WEAPON_GROUP);

        // Actors коллайдят с actors, weapons коллайдят с actors
        assert_eq!(actor_groups.filters, collision::ACTOR_GROUP);
        assert_eq!(weapon_groups.filters, collision::ACTOR_GROUP);
    }
}
