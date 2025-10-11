//! Weapon system - оружие и стрельба
//!
//! Architecture:
//! - ECS: Weapon state (cooldown, decisions)
//! - Godot: Aim execution (bone rotation), Fire visual (spawn projectile)
//! - Events: WeaponFired (ECS→Godot), ProjectileHit (Godot→ECS)

use bevy::prelude::*;

/// Компонент оружия (attached к актёру)
#[derive(Component, Debug, Clone, Reflect)]
pub struct Weapon {
    /// Урон за выстрел
    pub damage: u32,

    /// Cooldown между выстрелами (секунды)
    pub fire_cooldown: f32,

    /// Текущий cooldown timer (секунды)
    pub cooldown_timer: f32,

    /// Дальность выстрела (метры)
    pub range: f32,

    /// Скорость пули (м/с)
    pub projectile_speed: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 10,
            fire_cooldown: 0.5,
            cooldown_timer: 0.0,
            range: 20.0,
            projectile_speed: 30.0, // 8 м/с (медленнее для видимости)
        }
    }
}

impl Weapon {
    /// Может ли оружие стрелять (cooldown готов)
    pub fn can_fire(&self) -> bool {
        self.cooldown_timer <= 0.0
    }

    /// Начать cooldown после выстрела
    pub fn start_cooldown(&mut self) {
        self.cooldown_timer = self.fire_cooldown;
    }
}

// ❌ Projectile НЕ хранится в ECS — только в Godot (tactical layer)
// Godot полностью владеет lifecycle: spawn, physics, collision, cleanup
// ECS отвечает только за Weapon state и damage calculation

/// Event: Актёр стреляет (ECS → Godot)
/// ECS (strategic) принимает решение "стрелять в target"
/// Godot (tactical) рассчитывает точное direction из weapon bone (+Z axis)
#[derive(Event, Debug, Clone)]
pub struct WeaponFired {
    /// Кто стреляет
    pub shooter: Entity,

    /// В кого стреляет (для Godot aim calculation)
    pub target: Entity,

    /// Урон пули
    pub damage: u32,

    /// Скорость пули
    pub speed: f32,
}

/// Event: Projectile попал в цель (Godot → ECS)
#[derive(Event, Debug, Clone)]
pub struct ProjectileHit {
    /// Кто выстрелил (для предотвращения self-hit)
    pub shooter: Entity,

    /// В кого попали
    pub target: Entity,

    /// Урон
    pub damage: u32,
}

/// System: обновление weapon cooldowns
pub fn update_weapon_cooldowns(
    mut weapons: Query<&mut Weapon>,
    time: Res<Time>,
) {
    for mut weapon in weapons.iter_mut() {
        if weapon.cooldown_timer > 0.0 {
            weapon.cooldown_timer -= time.delta_secs();
        }
    }
}

/// System: AI weapon fire logic
/// Если актёр в Combat state и weapon ready → fire
pub fn ai_weapon_fire(
    mut actors: Query<(Entity, &crate::ai::AIState, &Transform, &mut Weapon)>,
    targets: Query<&Transform>,
    mut fire_events: EventWriter<WeaponFired>,
) {
    use crate::ai::AIState;

    for (entity, state, transform, mut weapon) in actors.iter_mut() {
        // Стреляем только в Combat state (Dead не стреляет)
        if let AIState::Combat { target } = state {
            // Проверяем cooldown
            if !weapon.can_fire() {
                continue;
            }

            // Проверяем дистанцию
            if let Ok(target_transform) = targets.get(*target) {
                let to_target = target_transform.translation - transform.translation;
                let distance = to_target.length();

                if distance <= weapon.range && distance > 0.01 {
                    // Генерируем событие выстрела (Godot рассчитает direction из weapon bone)
                    fire_events.write(WeaponFired {
                        shooter: entity,
                        target: *target,
                        damage: weapon.damage,
                        speed: weapon.projectile_speed,
                    });

                    // Начинаем cooldown
                    weapon.start_cooldown();

                    crate::log(&format!(
                        "Actor {:?} fires weapon at {:?} (distance: {:.1}m)",
                        entity, target, distance
                    ));
                }
            }
        }
    }
}

/// System: обработка ProjectileHit событий → нанесение урона
/// Godot отправляет событие после collision detection
pub fn process_projectile_hits(
    mut hit_events: EventReader<ProjectileHit>,
    mut targets: Query<&mut crate::Health>,
    mut damage_events: EventWriter<crate::combat::DamageDealt>,
) {
    for hit in hit_events.read() {
        crate::log(&format!(
            "🎯 ProjectileHit: shooter={:?} → target={:?} dmg={}",
            hit.shooter, hit.target, hit.damage
        ));

        // Проверка self-hit (не должно быть!)
        if hit.shooter == hit.target {
            crate::log(&format!(
                "⚠️ SELF-HIT DETECTED! Entity {:?} hit itself!",
                hit.shooter
            ));
            continue; // Пропускаем self-damage
        }

        // Наносим урон цели
        if let Ok(mut health) = targets.get_mut(hit.target) {
            let actual_damage = hit.damage.min(health.current);
            health.take_damage(actual_damage);

            let target_died = health.current == 0;

            // Генерируем DamageDealt event для визуальных эффектов
            damage_events.write(crate::combat::DamageDealt {
                attacker: hit.shooter,
                target: hit.target,
                damage: actual_damage,
                target_died,
            });

            crate::log(&format!(
                "💥 Projectile hit {:?} for {} damage (HP: {}, died: {})",
                hit.target, actual_damage, health.current, target_died
            ));
        }
    }
}

// ❌ cleanup_projectiles удалена — Godot полностью управляет lifecycle

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_cooldown() {
        let mut weapon = Weapon::default();
        assert!(weapon.can_fire());

        weapon.start_cooldown();
        assert!(!weapon.can_fire());
        assert_eq!(weapon.cooldown_timer, 0.5);
    }

    #[test]
    fn test_projectile_hit_event() {
        let shooter = Entity::PLACEHOLDER;
        let target = Entity::from_raw(1);

        let hit = ProjectileHit {
            shooter,
            target,
            damage: 20,
        };

        assert_eq!(hit.shooter, shooter);
        assert_eq!(hit.damage, 20);
    }
}
