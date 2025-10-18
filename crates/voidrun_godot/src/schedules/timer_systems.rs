//! Timer systems для tick-based schedules
//!
//! Системы запускаются в FixedUpdate (60 Hz) и управляют запуском
//! low-frequency schedules (SlowUpdate, CombatUpdate) через tick counter.

use bevy::prelude::{ResMut, World};
use super::{FixedTickCounter, SlowUpdate, CombatUpdate};

/// System: Increment tick counter (FixedUpdate, запускается ПЕРВЫМ)
///
/// Инкрементирует глобальный tick counter каждый FixedUpdate (60 Hz).
/// Wraparound safe: u64::MAX / 60 / 60 / 60 / 24 / 365 ≈ 9.7 миллиардов лет.
pub fn increment_tick_counter(mut counter: ResMut<FixedTickCounter>) {
    counter.tick = counter.tick.wrapping_add(1);  // Wraparound safe
}

/// System: Run SlowUpdate schedule каждые 20 ticks (3 Hz @ 60 Hz fixed)
///
/// Exclusive system (требует &mut World для run_schedule).
/// Запускается vision cone polling, target switching (человеческое время реакции ~0.3s).
pub fn run_slow_update_timer(world: &mut World) {
    let tick = world.resource::<FixedTickCounter>().tick;

    if tick % 20 == 0 {
        world.run_schedule(SlowUpdate);
    }
}

/// System: Run CombatUpdate schedule каждые 6 ticks (10 Hz @ 60 Hz fixed)
///
/// Exclusive system (требует &mut World для run_schedule).
/// Запускается windup detection, combat timing-sensitive mechanics (~0.1s window).
pub fn run_combat_update_timer(world: &mut World) {
    let tick = world.resource::<FixedTickCounter>().tick;

    if tick % 6 == 0 {
        world.run_schedule(CombatUpdate);
    }
}
