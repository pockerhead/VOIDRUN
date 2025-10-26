//! Combat integration test
//!
//! Milestone Фазы 1: 2 NPC дерутся headless 1000 тиков детерминистично
//!
//! Проверяем:
//! - Health/Stamina инварианты
//! - Детерминизм (3 прогона с seed=42)
//! - Нет паники/крашей

use bevy::prelude::*;
use voidrun_simulation::*;
use voidrun_simulation::ai::SpottedEnemies;

/// Helper: создать полный combat App с всеми plugins
fn create_combat_app(seed: u64) -> App {
    let mut app = create_headless_app(seed);

    // Используем SimulationPlugin (включает rapier + все подсистемы)
    app.add_plugins(SimulationPlugin);

    app
}

/// Helper: spawn NPC с AI
fn spawn_npc_fighter(commands: &mut Commands, position: Vec3, faction_id: u64) -> Entity {
    commands
        .spawn((
            // Transform
            Transform::from_translation(position),

            // Actor (Required: Health + Stamina)
            Actor { faction_id },

            // Combat

            // AI
            AIState::default(),
            AIConfig::default(),
            SpottedEnemies::default(),

            // Movement (Godot-driven)
            MovementCommand::Idle,
        ))
        .id()
}

/// Test: 2 NPC дерутся 1000 тиков без краша
#[test]
fn test_two_npcs_fight_1000_ticks() {
    let mut app = create_combat_app(42);

    // Spawn 2 NPC на расстоянии 5m (в радиусе detection 10m)
    let npc1 = spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(0.0, 0.0, 0.0),
        1, // Faction 1
    );
    let npc2 = spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(5.0, 0.0, 0.0),
        2, // Faction 2
    );

    // Прогоняем 1000 тиков (15.6 sec при 64Hz)
    for tick in 0..1000 {
        app.update();

        // Проверяем инварианты каждые 100 тиков
        if tick % 100 == 0 {
            check_invariants(&mut app, npc1, npc2, tick);
        }
    }

    crate::logger::log("✓ Combat integration test: 1000 ticks completed without crash");
}

/// Test: детерминизм — 3 прогона с seed=42 дают идентичные результаты
#[test]
fn test_combat_determinism_three_runs() {
    const SEED: u64 = 42;
    const TICKS: usize = 100; // Меньше для скорости теста

    // Прогоняем 3 раза
    let snapshot1 = run_combat_and_snapshot(SEED, TICKS);
    let snapshot2 = run_combat_and_snapshot(SEED, TICKS);
    let snapshot3 = run_combat_and_snapshot(SEED, TICKS);

    // Все снепшоты должны совпадать
    assert_eq!(
        snapshot1, snapshot2,
        "Combat determinism failed: run 1 != run 2"
    );
    assert_eq!(
        snapshot2, snapshot3,
        "Combat determinism failed: run 2 != run 3"
    );

    crate::logger::log(&format!("✓ Combat determinism: 3 runs with seed={} are identical", SEED));
}

/// Test: health/stamina инварианты сохраняются
#[test]
fn test_health_stamina_invariants() {
    let mut app = create_combat_app(123);

    // Spawn 2 NPC
    let npc1 = spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(0.0, 0.0, 0.0),
        1,
    );
    let npc2 = spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(5.0, 0.0, 0.0),
        2,
    );

    // Прогоняем 500 тиков
    for tick in 0..500 {
        app.update();

        // Проверяем инварианты каждый тик (строго)
        let world = app.world();

        // NPC 1
        if let Some(health) = world.get::<Health>(npc1) {
            assert!(
                health.current <= health.max,
                "Tick {}: NPC1 health.current ({}) > health.max ({})",
                tick,
                health.current,
                health.max
            );
        }
        if let Some(stamina) = world.get::<Stamina>(npc1) {
            assert!(
                stamina.current >= 0.0 && stamina.current <= stamina.max,
                "Tick {}: NPC1 stamina.current ({}) out of [0, {}]",
                tick,
                stamina.current,
                stamina.max
            );
        }

        // NPC 2
        if let Some(health) = world.get::<Health>(npc2) {
            assert!(
                health.current <= health.max,
                "Tick {}: NPC2 health.current ({}) > health.max ({})",
                tick,
                health.current,
                health.max
            );
        }
        if let Some(stamina) = world.get::<Stamina>(npc2) {
            assert!(
                stamina.current >= 0.0 && stamina.current <= stamina.max,
                "Tick {}: NPC2 stamina.current ({}) out of [0, {}]",
                tick,
                stamina.current,
                stamina.max
            );
        }
    }

    crate::logger::log("✓ Health/Stamina invariants: 500 ticks, all checks passed");
}

// --- Helpers ---

/// Проверка инвариантов для 2 NPC
fn check_invariants(app: &mut App, npc1: Entity, npc2: Entity, tick: usize) {
    let world = app.world();

    // Health инварианты
    if let Some(health) = world.get::<Health>(npc1) {
        assert!(
            health.current <= health.max,
            "Tick {}: NPC1 health invariant broken",
            tick
        );
    }
    if let Some(health) = world.get::<Health>(npc2) {
        assert!(
            health.current <= health.max,
            "Tick {}: NPC2 health invariant broken",
            tick
        );
    }

    // Stamina инварианты
    if let Some(stamina) = world.get::<Stamina>(npc1) {
        assert!(
            stamina.current >= 0.0 && stamina.current <= stamina.max,
            "Tick {}: NPC1 stamina invariant broken",
            tick
        );
    }
    if let Some(stamina) = world.get::<Stamina>(npc2) {
        assert!(
            stamina.current >= 0.0 && stamina.current <= stamina.max,
            "Tick {}: NPC2 stamina invariant broken",
            tick
        );
    }
}

/// Запускает combat симуляцию и возвращает snapshot
fn run_combat_and_snapshot(seed: u64, ticks: usize) -> Vec<u8> {
    let mut app = create_combat_app(seed);

    // Spawn 2 NPC
    spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(0.0, 0.0, 0.0),
        1,
    );
    spawn_npc_fighter(
        &mut app.world_mut().commands(),
        Vec3::new(5.0, 0.0, 0.0),
        2,
    );

    // Прогоняем ticks
    for _ in 0..ticks {
        app.update();
    }

    // Создаем snapshot (health + stamina + AIState)
    create_combat_snapshot(app.world_mut())
}

/// Создает snapshot состояния combat (health, stamina, AI state)
fn create_combat_snapshot(world: &mut World) -> Vec<u8> {
    let mut snapshot = Vec::new();

    // Собираем Health
    let mut health_query = world.query::<(Entity, &Health)>();
    let mut health_data: Vec<_> = health_query.iter(world).collect();
    health_data.sort_by_key(|(e, _)| e.index());
    for (entity, health) in health_data {
        snapshot.extend_from_slice(&entity.index().to_le_bytes());
        snapshot.extend_from_slice(&health.current.to_le_bytes());
        snapshot.extend_from_slice(&health.max.to_le_bytes());
    }

    // Собираем Stamina
    let mut stamina_query = world.query::<(Entity, &Stamina)>();
    let mut stamina_data: Vec<_> = stamina_query.iter(world).collect();
    stamina_data.sort_by_key(|(e, _)| e.index());
    for (entity, stamina) in stamina_data {
        snapshot.extend_from_slice(&entity.index().to_le_bytes());
        snapshot.extend_from_slice(&stamina.current.to_le_bytes());
        snapshot.extend_from_slice(&stamina.max.to_le_bytes());
    }

    // Собираем AIState (debug format для простоты)
    let mut ai_query = world.query::<(Entity, &AIState)>();
    let mut ai_data: Vec<_> = ai_query.iter(world).collect();
    ai_data.sort_by_key(|(e, _)| e.index());
    for (entity, state) in ai_data {
        snapshot.extend_from_slice(&entity.index().to_le_bytes());
        snapshot.extend_from_slice(format!("{:?}", state).as_bytes());
    }

    snapshot
}
