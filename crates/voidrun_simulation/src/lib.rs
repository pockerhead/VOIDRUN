//! VOIDRUN Simulation Core
//!
//! Детерминистичная ECS-симуляция на Bevy 0.16.
//! Архитектура: docs/architecture/bevy-ecs-design.md

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

// Публичные модули
pub mod components;
pub mod physics;
pub mod combat;
pub mod ai;
pub mod rollback;

// Re-export базовых компонентов для удобства
pub use components::{Actor, Health, Stamina, PhysicsBody};
pub use physics::{KinematicController, MovementInput, KinematicControllerPlugin, spawn_kinematic_character};
pub use combat::{
    CombatPlugin,
    AttackHitbox, Attacker, AttackStarted, HitboxOverlap,
    DamageDealt, EntityDied, calculate_damage,
    Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST,
    Weapon, WeaponState, spawn_weapon, collision,
};
pub use ai::{AIPlugin, AIState, AIConfig};
pub use rollback::Rollback;

/// Главный plugin симуляции (объединяет все подсистемы)
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app
            // Fixed timestep 64Hz для детерминизма
            .insert_resource(Time::<Fixed>::from_hz(64.0))
            // Детерминистичный RNG (seed по умолчанию)
            .insert_resource(DeterministicRng::new(42))
            // Rapier Physics (работает в FixedUpdate по умолчанию в 0.31)
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            // Подсистемы
            .add_plugins((
                KinematicControllerPlugin,
                CombatPlugin,
                AIPlugin,
            ));
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

pub static mut LOGGER: Option<Box<dyn LogPrinter>> = None;

pub fn set_logger(logger: Box<dyn LogPrinter>) {
    unsafe {
        LOGGER = Some(logger);
    }
}

pub trait LogPrinter {
    fn log(&self, message: &str);
}

pub fn log(message: &str) {
    unsafe {
        if let Some(logger) = LOGGER.as_ref() {
            logger.log(message);
        }
    }
}

struct ConsoleLogger;

impl LogPrinter for ConsoleLogger {
    fn log(&self, message: &str) {
        println!("{}", message);
    }
}

pub fn init_logger() {
    set_logger(Box::new(ConsoleLogger));
}