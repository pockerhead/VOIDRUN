//! VOIDRUN Simulation Core
//!
//! ECS-симуляция на Bevy 0.16 (strategic layer)
//! Архитектура: docs/architecture/bevy-ecs-design.md
//!
//! HYBRID ARCHITECTURE (ADR-003):
//! - ECS = strategic layer (game state, AI, combat rules)
//! - Godot = tactical layer (physics, rendering, pathfinding)

use bevy::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

// Публичные модули
pub mod ai;
pub mod combat;
pub mod components;

// Re-export базовых компонентов для удобства
pub use ai::{AIConfig, AIPlugin, AIState};
pub use combat::{
    calculate_damage, update_weapon_cooldowns, WeaponStats, WeaponType, CombatPlugin, DamageDealt, Dead, EntityDied,
    Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST,
};
pub use components::*;

// Re-export events
pub use components::movement::JumpIntent;

/// Главный plugin симуляции (объединяет все подсистемы)
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app
            // Fixed timestep 60Hz для simulation tick (легче считать интервалы)
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            // Детерминистичный RNG (seed по умолчанию)
            .insert_resource(DeterministicRng::new(42))
            // Подсистемы (ECS strategic layer)
            .add_plugins((CombatPlugin, AIPlugin));
    }
}

/// Детерминистичный RNG resource (seeded)
#[derive(Resource)]
pub struct DeterministicRng {
    pub rng: ChaCha8Rng,
    pub seed: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
        }
    }
}

/// Создаёт minimal Bevy App для headless симуляции
pub fn create_headless_app(seed: u64) -> App {
    let mut app = App::new();
    init_logger();
    app.add_plugins(MinimalPlugins)
        .insert_resource(DeterministicRng::new(seed))
        .insert_resource(Time::<Fixed>::from_hz(60.0)); // 60Hz FixedUpdate

    app
}

/// Snapshot мира для сравнения детерминизма
/// (упрощённая версия, полная в bevy_save будет позже)
pub fn world_snapshot<T: Component>(world: &mut World) -> Vec<u8>
where
    T: std::fmt::Debug,
{
    // Собираем все компоненты в детерминированный формат
    let mut snapshot = Vec::new();

    let mut query = world.query::<(Entity, &T)>();
    let mut entities: Vec<_> = query.iter(world).collect();

    // Сортируем по Entity ID для детерминизма
    entities.sort_by_key(|(entity, _)| entity.index());

    // Сериализуем в байты через Debug (простейший способ)
    for (entity, component) in entities {
        snapshot.extend_from_slice(&entity.index().to_le_bytes());
        snapshot.extend_from_slice(format!("{:?}", component).as_bytes());
    }

    snapshot
}

use once_cell::sync::Lazy;
use std::sync::Mutex;

// Потокобезопасный глобальный logger (упростили: убрали Arc, он не нужен для static)
static LOGGER: Lazy<Mutex<Option<Box<dyn LogPrinter>>>> =
    Lazy::new(|| Mutex::new(None));

pub static LOGGER_LEVEL: Lazy<Mutex<LogLevel>> = Lazy::new(|| Mutex::new(LogLevel::Debug));

pub fn set_logger(logger: Box<dyn LogPrinter>) {
    *LOGGER.lock().unwrap() = Some(logger);
}

pub fn set_log_level(level: LogLevel) {
    *LOGGER_LEVEL.lock().unwrap() = level;
}

pub fn set_logger_if_needed(logger: Box<dyn LogPrinter>) {
    if LOGGER.lock().unwrap().is_none() {
        set_logger(logger);
    }
}

pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl PartialOrd for LogLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_int().cmp(&other.as_int())
    }
}

impl PartialEq for LogLevel {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for LogLevel {}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        }
    }
}

pub trait LogPrinter: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
}

pub fn log(message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    log_with_level(LogLevel::Debug, message);
}

pub fn log_info(message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    log_with_level(LogLevel::Info, message);
}

pub fn log_warning(message: &str) {
    log_with_level(LogLevel::Warning, message);
}

pub fn log_error(message: &str) {
    log_with_level(LogLevel::Error, message);
}

pub fn log_with_level(level: LogLevel, message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        logger.log(level, &format!("[{}] {}", timestamp, message));
    }
}

struct ConsoleLogger;

impl LogPrinter for ConsoleLogger {
    fn log(&self, level: LogLevel, message: &str) {
        println!("[{}] {}", level.as_str(), message);
    }
}

pub fn init_logger() {
    set_logger_if_needed(Box::new(ConsoleLogger));
}
