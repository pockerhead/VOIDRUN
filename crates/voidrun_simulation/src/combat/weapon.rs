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

    /// Радиус слышимости выстрела (метры)
    /// Все актёры в этом радиусе слышат звук и реагируют
    pub hearing_range: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 10,
            fire_cooldown: 0.5,
            cooldown_timer: 0.0,
            range: 20.0,
            projectile_speed: 30.0, // 8 м/с (медленнее для видимости)
            hearing_range: 100.0, // 25м радиус слышимости для стандартного оружия
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

/// Event: Актёр ХОЧЕТ выстрелить (ECS strategic intent)
/// ECS принимает strategic decision: "cooldown готов, target в Combat state"
/// Godot validation проверяет tactical constraints: distance, LOS
#[derive(Event, Debug, Clone)]
pub struct WeaponFireIntent {
    /// Кто хочет стрелять
    pub shooter: Entity,

    /// В кого хочет стрелять
    pub target: Entity,

    /// Урон (из Weapon component)
    pub damage: u32,

    /// Скорость пули (из Weapon component)
    pub speed: f32,

    /// Max range (из Weapon component)
    pub max_range: f32,

    /// Радиус слышимости выстрела (для AI reaction)
    pub hearing_range: f32,
}

/// Event: Актёр стреляет (ECS → Godot, после validation)
/// Godot tactical layer проверил distance/LOS и разрешил выстрел
/// Godot рассчитывает точное direction из weapon bone (+Z axis)
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

    /// Позиция стрелявшего (Godot Transform, для AI sound reaction)
    pub shooter_position: Vec3,

    /// Радиус слышимости выстрела (для AI reaction)
    pub hearing_range: f32,
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

/// System: AI weapon fire intent (ECS strategic decision)
///
/// Архитектура (Hybrid Intent-based):
/// 1. ECS (strategic): Проверяет cooldown + AI state → генерирует WeaponFireIntent
/// 2. Godot (tactical): Проверяет distance/LOS → конвертирует Intent → WeaponFired
///
/// Почему так:
/// - ECS не знает точных Godot positions (только chunk-based StrategicPosition)
/// - Godot authoritative для tactical validation (distance, line of sight)
/// - Разделение ответственности: strategic intent vs tactical execution
pub fn ai_weapon_fire_intent(
    mut actors: Query<(Entity, &crate::ai::AIState, &mut Weapon)>,
    mut intent_events: EventWriter<WeaponFireIntent>,
) {
    use crate::ai::AIState;

    for (entity, state, mut weapon) in actors.iter_mut() {
        // Стреляем только в Combat state
        let AIState::Combat { target } = state else {
            continue;
        };

        // Проверяем cooldown (strategic constraint)
        if !weapon.can_fire() {
            continue;
        }

        // Генерируем intent (Godot проверит distance/LOS)
        intent_events.write(WeaponFireIntent {
            shooter: entity,
            target: *target,
            damage: weapon.damage,
            speed: weapon.projectile_speed,
            max_range: weapon.range,
            hearing_range: weapon.hearing_range,
        });

        // Начинаем cooldown (ECS владеет cooldown state)
        weapon.start_cooldown();

        crate::log(&format!(
            "Actor {:?} wants to fire at {:?} (intent generated)",
            entity, target
        ));
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
