//! Stamina management systems.

use bevy::prelude::*;
use crate::components::Stamina;
use crate::combat::components::stamina::Exhausted;

/// Стоимость различных действий (stamina points)
pub const ATTACK_COST: f32 = 30.0;
pub const BLOCK_COST: f32 = 20.0;
pub const DODGE_COST: f32 = 25.0; // Для будущего

/// Система: regenerate stamina для всех entities
///
/// Работает в FixedUpdate для детерминизма.
/// Regen rate берется из Stamina::regen_rate (default 10.0 units/sec).
pub fn regenerate_stamina(
    mut query: Query<&mut Stamina>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for mut stamina in query.iter_mut() {
        stamina.regenerate(delta);
    }
}

/// Система: consume stamina при атаках (placeholder)
///
/// TODO: Будет слушать GodotAnimationEvent::AnimationTrigger("attack_start")
/// Godot AnimationTree trigger → ECS consume stamina
pub fn consume_stamina_on_attack(
    // TODO: mut animation_events: EventReader<GodotAnimationEvent>,
    mut _attackers: Query<&mut Stamina>,
) {
    // Stub для компиляции
    // Реальная логика будет после Godot integration
}

/// Система: detect exhaustion (stamina < 20%)
///
/// Добавляет Exhausted компонент когда stamina низкая.
/// Убирает когда восстановилась > 50%.
pub fn detect_exhaustion(
    mut commands: Commands,
    query: Query<(Entity, &Stamina, Option<&Exhausted>)>,
) {
    for (entity, stamina, exhausted) in query.iter() {
        let stamina_percent = stamina.current / stamina.max;

        if exhausted.is_none() && stamina_percent < 0.2 {
            // Стал exhausted
            commands.entity(entity).insert(Exhausted::default());
            // Debug logging
            // eprintln!("DEBUG: Entity {:?} is now exhausted (stamina: {:.1}%)", entity, stamina_percent * 100.0);
        } else if exhausted.is_some() && stamina_percent > 0.5 {
            // Восстановился
            commands.entity(entity).remove::<Exhausted>();
            // eprintln!("DEBUG: Entity {:?} recovered from exhaustion", entity);
        }
    }
}
