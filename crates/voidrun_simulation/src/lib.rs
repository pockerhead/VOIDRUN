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

// Публичные модули (domains)
pub mod ai;
pub mod logger;
pub mod combat;
pub mod equipment;
pub mod item_system;
pub mod player;

// New domains (Phase 1 refactoring)
pub mod actor;
pub mod movement;
pub mod shooting;
pub mod shared;

// Legacy components module (re-exports from domains for backward compatibility)
pub mod components;

// Re-export базовых компонентов для удобства
pub use ai::{AIConfig, AIPlugin, AIState};
pub use combat::{
    calculate_damage, update_weapon_cooldowns, WeaponStats, WeaponType, CombatPlugin, DamageDealt, Dead, EntityDied,
    Exhausted, ATTACK_COST, BLOCK_COST, DODGE_COST,
};
pub use components::*;
pub use item_system::{
    ArmorStatsTemplate, ConsumableEffect, ItemDefinition, ItemDefinitions, ItemId, ItemInstance,
    ItemType, WeaponSize, WeaponStatsTemplate,
};
pub use equipment::{
    EquipWeaponIntent, UnequipWeaponIntent, SwapActiveWeaponIntent, WeaponSlot,
    EquipArmorIntent, UnequipArmorIntent, UseConsumableIntent, EquipmentPlugin,
};

// Re-export events
pub use movement::JumpIntent;
pub use shooting::ToggleADSIntent;

/// Главный plugin симуляции (объединяет все подсистемы)
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app
            // Fixed timestep 60Hz для simulation tick (легче считать интервалы)
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            // Детерминистичный RNG (seed по умолчанию)
            .insert_resource(DeterministicRng::new(42))
            // Item definitions (hardcoded базовые items)
            .insert_resource(ItemDefinitions::default())
            // Подсистемы (ECS strategic layer)
            .add_plugins((CombatPlugin, AIPlugin, EquipmentPlugin));
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
    logger::init_logger();
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