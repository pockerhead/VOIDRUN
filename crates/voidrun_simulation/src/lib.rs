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
    calculate_damage, tick_attack_cooldowns, Attacker, CombatPlugin, DamageDealt, Dead, EntityDied,
    Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST,
};
pub use components::{
    Actor, Attachment, AttachmentType, DetachAttachment, Health, MovementCommand, Stamina,
};

/// Главный plugin симуляции (объединяет все подсистемы)
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app
            // Fixed timestep 64Hz для simulation tick
            .insert_resource(Time::<Fixed>::from_hz(64.0))
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
        .insert_resource(Time::<Fixed>::from_hz(64.0)); // 64Hz FixedUpdate

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

pub fn set_logger(logger: Box<dyn LogPrinter>) {
    *LOGGER.lock().unwrap() = Some(logger);
}

pub fn set_logger_if_needed(logger: Box<dyn LogPrinter>) {
    if LOGGER.lock().unwrap().is_none() {
        set_logger(logger);
    }
}

pub trait LogPrinter: Send + Sync {
    fn log(&self, message: &str);
}

pub fn log(message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        logger.log(&format!("[{}] {}", timestamp, message));
    }
}

struct ConsoleLogger;

impl LogPrinter for ConsoleLogger {
    fn log(&self, message: &str) {
        println!("{}", message);
    }
}

pub fn init_logger() {
    set_logger_if_needed(Box::new(ConsoleLogger));
}
