//! Property-based тесты детерминизма
//!
//! Проверяем что симуляция с одинаковым seed даёт идентичные результаты

use bevy::prelude::*;
use voidrun_simulation::{create_headless_app, world_snapshot};

/// Тестовый компонент для симуляции движения
#[derive(Component, Debug)]
struct TestEntity {
    x: f32,
    y: f32,
}

/// Система движения для тестовых сущностей
fn move_entities(mut query: Query<&mut TestEntity>) {
    for mut entity in query.iter_mut() {
        entity.x += 0.1;
        entity.y += 0.05;
    }
}

#[test]
fn test_determinism_same_seed() {
    const SEED: u64 = 12345;
    const ENTITY_COUNT: usize = 100;
    const TICK_COUNT: usize = 1000;

    // Первый прогон
    let snapshot1 = run_simulation(SEED, ENTITY_COUNT, TICK_COUNT);

    // Второй прогон с тем же seed
    let snapshot2 = run_simulation(SEED, ENTITY_COUNT, TICK_COUNT);

    // Снепшоты должны быть идентичны
    assert_eq!(
        snapshot1, snapshot2,
        "Симуляция с одинаковым seed ({}) дала разные результаты!",
        SEED
    );
}

#[test]
fn test_determinism_multiple_runs() {
    const SEED: u64 = 42;
    const ENTITY_COUNT: usize = 100;
    const TICK_COUNT: usize = 1000;

    // Запускаем 5 раз — все должны быть идентичны
    let snapshots: Vec<_> = (0..5)
        .map(|_| run_simulation(SEED, ENTITY_COUNT, TICK_COUNT))
        .collect();

    // Все снепшоты должны совпадать с первым
    for (i, snapshot) in snapshots.iter().enumerate().skip(1) {
        assert_eq!(
            snapshots[0], *snapshot,
            "Прогон {} дал результат отличный от прогона 0",
            i
        );
    }
}

/// Запускает симуляцию и возвращает snapshot мира
fn run_simulation(seed: u64, entity_count: usize, tick_count: usize) -> Vec<u8> {
    let mut app = create_headless_app(seed);

    // Добавляем систему движения
    app.add_systems(FixedUpdate, move_entities);

    // Спавним тестовые entities
    for i in 0..entity_count {
        app.world_mut().spawn(TestEntity {
            x: i as f32,
            y: i as f32 * 0.5,
        });
    }

    // Прогоняем симуляцию
    for _ in 0..tick_count {
        app.update();
    }

    // Возвращаем snapshot
    world_snapshot::<TestEntity>(app.world_mut())
}
