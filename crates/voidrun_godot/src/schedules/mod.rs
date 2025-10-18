//! Custom schedules and tick counter
//!
//! Tick-based scheduling для детерминистичных low-frequency updates.
//!
//! # Архитектура
//!
//! **FixedUpdate (60 Hz)** → increment_tick_counter
//!   ├─ tick % 20 == 0 → SlowUpdate (3 Hz)
//!   └─ tick % 6 == 0 → CombatUpdate (10 Hz)
//!
//! # Почему tick-based, а не on_timer()?
//!
//! - **Детерминизм:** Tick counter инкрементируется в FixedUpdate (не зависит от FPS)
//! - **Точность:** Modulo не дрейфует (в отличие от timer += delta)
//! - **Wraparound safe:** u64::MAX / 60 / 60 / 60 / 24 / 365 ≈ 9.7 миллиардов лет

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::Resource;

pub mod timer_systems;

/// Глобальный tick counter (детерминистичный, wraparound safe)
///
/// Инкрементируется в каждый FixedUpdate tick (60 Hz).
/// Используется для запуска low-frequency schedules (SlowUpdate, CombatUpdate).
///
/// # Overflow Protection
/// u64::MAX / 60 / 60 / 60 / 24 / 365 ≈ 9.7 миллиардов лет.
/// Wraparound safe: modulo автоматически handle overflow.
#[derive(Resource, Default)]
pub struct FixedTickCounter {
    pub tick: u64,
}

/// Custom schedule: SlowUpdate (3 Hz = 60/20)
///
/// Для систем с "человеческим временем реакции":
/// - Vision cone polling (poll_vision_cones_main_thread)
/// - Target switching (update_combat_targets_main_thread)
/// - AI decision making (low priority)
///
/// Запускается каждые 20 ticks (60 Hz / 20 = 3 Hz = 0.333s)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlowUpdate;

/// Custom schedule: CombatUpdate (10 Hz = 60/6)
///
/// Для combat-критичных систем с быстрой реакцией:
/// - Windup detection (detect_melee_windups_main_thread) - ФАЗА 3
/// - Combat events processing
/// - Timing-sensitive mechanics
///
/// Запускается каждые 6 ticks (60 Hz / 6 = 10 Hz = 0.1s)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CombatUpdate;
