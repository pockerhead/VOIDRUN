# ADR-006: Chunk-based Streaming World (Procgen)

**Дата:** 2025-01-10
**Статус:** ✅ ПРИНЯТО
**Связанные ADR:** [ADR-005](ADR-005-transform-ownership-strategic-positioning.md)

## Контекст

Из требований пользователя:

> "у меня нет ресурсов чтобы делать целые уровни и расставлять там врагов - все должна решать процедурка"
>
> "как в minecraft в идеале - новые чанки подгружаются когда игрок (игроки в ММО) их достигает"
>
> "важно сохранить всё что требуется для того чтобы при загрузке всё и все были на своих местах"

### Требования

1. **Процедурная генерация** — уровни создаются runtime, не в редакторе
2. **Бесконечный мир** — как Minecraft, новые чанки генерируются по мере исследования
3. **Streaming** — подгружаем только видимое, выгружаем далёкое
4. **Детерминизм** — один seed = одинаковый мир
5. **Компактные saves** — не храним всю вселенную, только изменения
6. **MMO-ready** — архитектура должна масштабироваться на множество игроков

## Решение

**Chunk-based streaming world с детерминированной procgen и delta saves**

### Концепция

**Chunk** — базовая единица генерации, загрузки и выгрузки:

```
Вселенная = бесконечная сетка чанков

┌─────┬─────┬─────┬─────┬─────┐
│-2,2 │-1,2 │ 0,2 │ 1,2 │ 2,2 │  ← Unloaded (сохранены на диске)
├─────┼─────┼─────┼─────┼─────┤
│-2,1 │-1,1 │ 0,1 │ 1,1 │ 2,1 │  ← Loaded (вокруг игрока)
├─────┼─────┼─────┼─────┼─────┤
│-2,0 │-1,0 │ 0,0 │ 1,0 │ 2,0 │  ← Player в (0,0)
├─────┼─────┼─────┼─────┼─────┤
│-2,-1│-1,-1│ 0,-1│ 1,-1│ 2,-1│
├─────┼─────┼─────┼─────┼─────┤
│-2,-2│-1,-2│ 0,-2│ 1,-2│ 2,-2│
└─────┴─────┴─────┴─────┴─────┘

Load radius = 1 → 9 chunks активны (3x3)
Load radius = 2 → 25 chunks активны (5x5)
```

**ChunkCoord** = `IVec2(x, y)` в бесконечной сетке.

### Chunk Size

**Рекомендация:**

- **32x32 метра** для interior (космическая станция, корабли)
- **128x128 метров** для exterior (планеты, астероиды)

**Обоснование:**

| Размер | Traversal time | Комнат | Entities | Плюсы | Минусы |
|--------|----------------|--------|----------|-------|--------|
| 8x8м | ~3 сек | 0.5 | 1-3 | Мелкая granularity | Слишком много загрузок |
| 16x16м | ~6 сек | 1 | 3-5 | OK granularity | Много чанков в памяти |
| **32x32м** | **~10-15 сек** | **1-2** | **5-10** | **Sweet spot** | - |
| 64x64м | ~30 сек | 4-8 | 20-40 | Меньше загрузок | Слишком крупно для interior |
| 128x128м | ~60 сек | 16+ | 100+ | OK для планет | Слишком крупно для станций |

**Выбор:** `const CHUNK_SIZE: f32 = 32.0` (configurable в config файле).

### Load Radius

**Рекомендация:** `load_radius = 1` (9 чанков = 3x3).

**Почему:**
- Видимость ~64-96 метров (достаточно для interior gameplay)
- Memory footprint: 9 чанков × ~10 entities × ~500 bytes = ~45 KB (минимально)
- Можно увеличить для exterior (radius = 2-3)

**Configurable:**
```rust
#[derive(Resource)]
pub struct ChunkLoadSettings {
    pub load_radius: i32,      // 1 для interior, 2-3 для exterior
    pub unload_delay: f32,     // 5.0 секунд задержка перед unload (hysteresis)
}
```

## Архитектура

### ECS: Chunk Management

```rust
// === voidrun_simulation/src/world/chunk.rs ===

pub type ChunkCoord = IVec2; // (x, y) в сетке чанков

/// Resource: загруженные чанки
#[derive(Resource)]
pub struct LoadedChunks {
    pub chunks: HashMap<ChunkCoord, ChunkData>,
    pub load_radius: i32,
}

/// Данные одного чанка
pub struct ChunkData {
    pub coord: ChunkCoord,
    pub biome: BiomeType,
    pub entities: Vec<Entity>,           // Entities spawned в этом чанке
    pub generated: bool,                 // Procgen выполнена?
    pub loaded_in_godot: bool,           // Геометрия загружена в Godot?
    pub last_visited: f64,               // Timestamp для unload hysteresis
}

/// Биомы (типы чанков)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiomeType {
    // Interior (станция)
    Station_Corridor,
    Station_Warehouse,
    Station_Reactor,
    Station_MedBay,
    Station_Engineering,

    // Exterior (планеты)
    Planet_Desert,
    Planet_Forest,
    Planet_Ice,
    Planet_Lava,

    // Space
    Asteroid,
    DerelictShip,
}

/// World seed (для детерминизма)
#[derive(Resource)]
pub struct WorldSeed(pub u64);

/// Маркер принадлежности к чанку
#[derive(Component)]
pub struct ParentChunk(pub ChunkCoord);
```

### Core System: update_chunk_loading

```rust
// === voidrun_simulation/src/world/chunk_manager.rs ===

/// Система загрузки/выгрузки чанков вокруг игрока
pub fn update_chunk_loading(
    player_query: Query<&StrategicPosition, With<Player>>,
    mut loaded_chunks: ResMut<LoadedChunks>,
    mut commands: Commands,
    seed: Res<WorldSeed>,
    time: Res<Time>,
    settings: Res<ChunkLoadSettings>,
) {
    let player_pos = player_query.single();
    let player_chunk = player_pos.chunk;

    // === 1. Определить какие чанки нужны ===
    let mut required_chunks = HashSet::new();
    let radius = loaded_chunks.load_radius;

    for dx in -radius..=radius {
        for dy in -radius..=radius {
            required_chunks.insert(ChunkCoord::new(
                player_chunk.x + dx,
                player_chunk.y + dy,
            ));
        }
    }

    // === 2. Выгрузить чанки вне радиуса ===
    loaded_chunks.chunks.retain(|coord, chunk_data| {
        if !required_chunks.contains(coord) {
            // Hysteresis: не выгружать сразу, подождать N секунд
            let time_since_visit = time.elapsed_seconds_f64() - chunk_data.last_visited;
            if time_since_visit < settings.unload_delay as f64 {
                return true; // Ещё не время выгружать
            }

            info!("Unloading chunk {:?}", coord);

            // Сохранить состояние чанка на диск (delta от procgen)
            save_chunk_to_disk(chunk_data, &seed);

            // Despawn entities из чанка
            for entity in &chunk_data.entities {
                commands.entity(*entity).despawn_recursive();
            }

            // Event для Godot: выгрузить геометрию
            commands.add(|world: &mut World| {
                world.send_event(ChunkEvent::Unload { coord: *coord });
            });

            false // Remove from HashMap
        } else {
            // Обновить last_visited
            chunk_data.last_visited = time.elapsed_seconds_f64();
            true // Keep
        }
    });

    // === 3. Загрузить новые чанки ===
    for coord in required_chunks {
        if !loaded_chunks.chunks.contains_key(&coord) {
            info!("Loading chunk {:?}", coord);

            // Проверить есть ли сохранённый чанк
            let chunk_data = if let Some(saved) = load_chunk_from_disk(coord) {
                info!("  → Loaded from disk (modified)");
                saved
            } else {
                info!("  → Generating new chunk (procgen)");
                // Генерация нового чанка (детерминированно от seed)
                generate_chunk(coord, seed.0)
            };

            // Spawn entities из чанка
            spawn_chunk_entities(&mut commands, &chunk_data);

            // Event для Godot: загрузить геометрию
            let biome = chunk_data.biome;
            commands.add(move |world: &mut World| {
                world.send_event(ChunkEvent::Load { coord, biome });
            });

            loaded_chunks.chunks.insert(coord, chunk_data);
        }
    }
}
```

### Детерминированная генерация

```rust
// === voidrun_simulation/src/world/procgen.rs ===

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// Хеш chunk координат + world seed → уникальный seed для чанка
fn hash_chunk_coord(coord: ChunkCoord, world_seed: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    world_seed.hash(&mut hasher);
    coord.x.hash(&mut hasher);
    coord.y.hash(&mut hasher);
    hasher.finish()
}

/// Генерация чанка (детерминированно)
pub fn generate_chunk(coord: ChunkCoord, world_seed: u64) -> ChunkData {
    // Уникальный seed для этого чанка
    let chunk_seed = hash_chunk_coord(coord, world_seed);
    let mut rng = StdRng::seed_from_u64(chunk_seed);

    // === 1. Определить биом ===
    let biome = determine_biome(coord, world_seed, &mut rng);

    // === 2. Сгенерировать контент (НЕ spawn сразу, только данные) ===
    let enemy_count = match biome {
        BiomeType::Station_Corridor => rng.gen_range(1..3),
        BiomeType::Station_Warehouse => rng.gen_range(3..8),
        BiomeType::Station_Reactor => rng.gen_range(5..10),
        BiomeType::Planet_Desert => rng.gen_range(2..5),
        _ => 0,
    };

    let loot_count = match biome {
        BiomeType::Station_Warehouse => rng.gen_range(5..15),
        BiomeType::Station_MedBay => rng.gen_range(3..8),
        _ => rng.gen_range(0..3),
    };

    ChunkData {
        coord,
        biome,
        entities: Vec::new(), // Заполнится в spawn_chunk_entities
        generated: true,
        loaded_in_godot: false,
        last_visited: 0.0,
        // Храним параметры генерации (для delta calculation)
        procgen_params: ProcgenParams {
            enemy_count,
            loot_count,
        },
    }
}

/// Определить биом (Perlin noise)
fn determine_biome(coord: ChunkCoord, world_seed: u64, rng: &mut StdRng) -> BiomeType {
    // Perlin noise для smooth transitions между биомами
    let noise = perlin_2d(coord.x as f64 * 0.1, coord.y as f64 * 0.1, world_seed);

    match noise {
        n if n < -0.5 => BiomeType::Station_Reactor,
        n if n < -0.2 => BiomeType::Station_Engineering,
        n if n < 0.2 => BiomeType::Station_Corridor,
        n if n < 0.5 => BiomeType::Station_Warehouse,
        _ => BiomeType::Station_MedBay,
    }
}

// Простой Perlin noise (можно использовать библиотеку noise-rs)
fn perlin_2d(x: f64, y: f64, seed: u64) -> f64 {
    // TODO: Implement или использовать noise-rs crate
    use noise::{NoiseFn, Perlin};
    let perlin = Perlin::new(seed as u32);
    perlin.get([x, y])
}

/// Spawn entities для чанка
fn spawn_chunk_entities(commands: &mut Commands, chunk_data: &ChunkData) {
    let chunk_seed = hash_chunk_coord(chunk_data.coord, 12345); // Используем фиксированный seed для consistent spawn
    let mut rng = StdRng::seed_from_u64(chunk_seed);

    const CHUNK_SIZE: f32 = 32.0;

    // Spawn врагов
    for _ in 0..chunk_data.procgen_params.enemy_count {
        let entity = commands.spawn((
            StrategicPosition {
                chunk: chunk_data.coord,
                local_offset: Vec2::new(
                    rng.gen_range(0.0..CHUNK_SIZE),
                    rng.gen_range(0.0..CHUNK_SIZE),
                ),
            },
            Health { current: 100.0, max: 100.0 },
            AIState::Idle,
            VisualPrefab { path: "res://prefabs/enemy_grunt.tscn".into() },
            ParentChunk(chunk_data.coord),
        )).id();

        // Запомнить entity (для выгрузки позже)
        // NOTE: chunk_data borrowed immutably, нужно другой подход
        // (например вернуть Vec<Entity> из функции)
    }

    // Spawn лута
    for _ in 0..chunk_data.procgen_params.loot_count {
        commands.spawn((
            StrategicPosition {
                chunk: chunk_data.coord,
                local_offset: Vec2::new(
                    rng.gen_range(0.0..CHUNK_SIZE),
                    rng.gen_range(0.0..CHUNK_SIZE),
                ),
            },
            Item { id: ItemId::MedKit, quantity: 1 },
            VisualPrefab { path: "res://prefabs/items/medkit.tscn".into() },
            ParentChunk(chunk_data.coord),
        ));
    }
}
```

**Гарантия детерминизма:**

```rust
#[test]
fn test_chunk_generation_deterministic() {
    let seed = 12345u64;
    let coord = ChunkCoord::new(5, 10);

    let chunk1 = generate_chunk(coord, seed);
    let chunk2 = generate_chunk(coord, seed);

    assert_eq!(chunk1.biome, chunk2.biome);
    assert_eq!(chunk1.procgen_params.enemy_count, chunk2.procgen_params.enemy_count);
    assert_eq!(chunk1.procgen_params.loot_count, chunk2.procgen_params.loot_count);
}
```

### Godot: Geometry Loading

```rust
// === voidrun_godot/src/world/chunk_loader.rs ===

/// События загрузки/выгрузки чанков
#[derive(Event, Clone, Debug)]
pub enum ChunkEvent {
    Load { coord: ChunkCoord, biome: BiomeType },
    Unload { coord: ChunkCoord },
}

/// Resource: загруженная геометрия чанков
#[derive(Resource)]
pub struct ChunkGeometry {
    /// ChunkCoord → Godot Node3D (корень чанка)
    pub loaded: HashMap<ChunkCoord, Gd<Node3D>>,
}

/// Resource: NavigationRegion для каждого чанка
#[derive(Resource)]
pub struct ZoneRegistry {
    /// ChunkCoord → NavigationRegion3D (для spawn NPC)
    pub nav_regions: HashMap<ChunkCoord, Gd<NavigationRegion3D>>,
}

/// Система обработки событий загрузки чанков
pub fn process_chunk_events(
    mut events: EventReader<ChunkEvent>,
    mut geometry: ResMut<ChunkGeometry>,
    mut zones: ResMut<ZoneRegistry>,
    scene_root: Res<GodotSceneRoot>,
) {
    for event in events.read() {
        match event {
            ChunkEvent::Load { coord, biome } => {
                info!("Godot: Loading chunk geometry {:?} (biome {:?})", coord, biome);

                // === 1. Выбрать prefab по биому ===
                let prefab_path = match biome {
                    BiomeType::Station_Corridor => "res://chunks/corridor_chunk.tscn",
                    BiomeType::Station_Warehouse => "res://chunks/warehouse_chunk.tscn",
                    BiomeType::Station_Reactor => "res://chunks/reactor_chunk.tscn",
                    BiomeType::Station_MedBay => "res://chunks/medbay_chunk.tscn",
                    BiomeType::Planet_Desert => "res://chunks/planet_desert_tile.tscn",
                    _ => "res://chunks/default_chunk.tscn",
                };

                // === 2. Загрузить Godot scene ===
                let scene = load::<PackedScene>(prefab_path);
                let mut chunk_node = scene.instantiate_as::<Node3D>();

                // === 3. Позиция чанка в мире ===
                const CHUNK_SIZE: f32 = 32.0;
                let world_pos = Vector3::new(
                    coord.x as f32 * CHUNK_SIZE,
                    0.0,
                    coord.y as f32 * CHUNK_SIZE,
                );
                chunk_node.set_position(world_pos);

                // === 4. Добавить в сцену ===
                scene_root.add_child(chunk_node.clone());

                // === 5. Зарегистрировать NavigationRegion ===
                if let Some(nav_region) = chunk_node.try_get_node_as::<NavigationRegion3D>("NavigationRegion3D") {
                    zones.nav_regions.insert(*coord, nav_region);

                    // Bake NavigationMesh (если ещё не baked)
                    // NOTE: В Godot 4.x baking может быть автоматическим
                }

                // === 6. Запомнить chunk node ===
                geometry.loaded.insert(*coord, chunk_node);
            }

            ChunkEvent::Unload { coord } => {
                info!("Godot: Unloading chunk geometry {:?}", coord);

                // Удалить геометрию из сцены
                if let Some(mut chunk_node) = geometry.loaded.remove(coord) {
                    chunk_node.queue_free();
                }

                // Удалить из zone registry
                zones.nav_regions.remove(coord);
            }
        }
    }
}

/// Spawn NPC в только что загруженных чанках
pub fn spawn_entities_in_loaded_chunks(
    // Added<StrategicPosition> = только что заспавнены в ECS
    query: Query<(Entity, &StrategicPosition, &VisualPrefab), Added<StrategicPosition>>,
    zones: Res<ZoneRegistry>,
    mut visuals: ResMut<VisualRegistry>,
) {
    for (entity, strategic_pos, prefab) in query.iter() {
        // Получить NavigationRegion для чанка
        let nav_region = match zones.nav_regions.get(&strategic_pos.chunk) {
            Some(region) => region,
            None => {
                // Чанк ещё не загружен в Godot — пропустить
                // (будет обработан на следующем кадре)
                warn!("Chunk {:?} not loaded in Godot yet, skipping entity spawn", strategic_pos.chunk);
                continue;
            }
        };

        // === 1. Найти spawn point на NavMesh ===
        // Используем local_offset как hint
        const CHUNK_SIZE: f32 = 32.0;
        let hint_position = Vector3::new(
            strategic_pos.chunk.x as f32 * CHUNK_SIZE + strategic_pos.local_offset.x,
            0.0,
            strategic_pos.chunk.y as f32 * CHUNK_SIZE + strategic_pos.local_offset.y,
        );

        let spawn_position = find_nearest_navmesh_point(nav_region, hint_position);

        // === 2. Загрузить visual prefab ===
        let scene = load::<PackedScene>(&prefab.path);
        let mut instance = scene.instantiate_as::<CharacterBody3D>();

        // === 3. GODOT решает координаты (authoritative) ===
        instance.set_position(spawn_position);
        instance.set_meta("entity_id".into(), entity.index().to_variant());

        // === 4. Добавить в сцену ===
        nav_region.add_child(instance.clone());

        // === 5. Зарегистрировать в визуал registry ===
        visuals.visuals.insert(entity, instance);
    }
}

/// Найти ближайшую точку на NavMesh
fn find_nearest_navmesh_point(nav_region: &Gd<NavigationRegion3D>, hint: Vector3) -> Vector3 {
    let nav_map = nav_region.get_navigation_map();
    let closest = NavigationServer3D::singleton().map_get_closest_point(nav_map, hint);

    // Если NavMesh не найден (ещё не baked) → вернуть hint
    if closest.distance_to(hint) > 100.0 {
        warn!("NavMesh not found or too far, using hint position");
        return hint;
    }

    closest
}
```

## Save/Load: Seed + Deltas

### Концепция

**НЕ сохраняем** всю вселенную (миллионы потенциальных чанков).

**Сохраняем:**
1. **World seed** (8 bytes) — для воспроизведения procgen
2. **Player data** (~200 bytes) — позиция, инвентарь, stats
3. **Chunk deltas** — только изменения от procgen baseline

**Пример:**
- Procgen сгенерировал 5 врагов в чанке (3, 7)
- Игрок убил 2 врагов, подобрал 3 предмета
- **Delta:** `removed_entities: [entity1, entity2]`, `removed_items: [item1, item2, item3]`

### Save File Structure

```rust
// === voidrun_simulation/src/save/mod.rs ===

#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    /// Version (для миграций)
    pub version: u32,

    /// World seed (для детерминированной генерации)
    pub world_seed: u64,

    /// Игрок
    pub player: PlayerSave,

    /// Изменения в чанках (только отличия от procgen)
    pub chunk_deltas: HashMap<ChunkCoord, ChunkDelta>,

    /// Метаданные
    pub meta: SaveMetadata,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerSave {
    pub position: StrategicPosition,
    pub health: f32,
    pub stamina: f32,
    pub inventory: Vec<ItemStack>,
    pub equipped_weapon: Option<ItemId>,
}

#[derive(Serialize, Deserialize)]
pub struct ChunkDelta {
    /// Entities убитые игроком (не respawn)
    /// Храним original procgen index (для идентификации)
    pub removed_entities: HashSet<u32>,

    /// Entities добавленные игроком (постройки, dropped items)
    pub added_entities: Vec<AddedEntityData>,

    /// Entities с изменённым состоянием (например NPC ранен)
    pub modified_entities: Vec<ModifiedEntityData>,

    /// Собранные items (не respawn)
    pub removed_items: HashSet<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct AddedEntityData {
    pub archetype: String,              // "BuildingFoundation", "DroppedItem"
    pub position: StrategicPosition,
    pub components: HashMap<String, Vec<u8>>, // Serialized components (bincode)
}

#[derive(Serialize, Deserialize)]
pub struct ModifiedEntityData {
    pub original_index: u32,            // Index procgen entity (для идентификации)
    pub health: Option<f32>,            // Если изменилось
    pub position: Option<StrategicPosition>, // Если переместилось
    pub ai_state: Option<AIState>,      // Если изменилось
}

#[derive(Serialize, Deserialize)]
pub struct SaveMetadata {
    pub playtime: f64,                  // Секунд игры
    pub timestamp: u64,                 // Unix timestamp
    pub game_version: String,           // "0.1.0"
}
```

### Save процесс

```rust
// === voidrun_simulation/src/save/save.rs ===

pub fn save_game(world: &World) -> Result<SaveFile, SaveError> {
    let loaded_chunks = world.resource::<LoadedChunks>();
    let world_seed = world.resource::<WorldSeed>().0;

    let mut chunk_deltas = HashMap::new();

    for (coord, chunk_data) in &loaded_chunks.chunks {
        // Пересоздать procgen baseline для этого чанка
        let original_chunk = generate_chunk(*coord, world_seed);

        // Вычислить delta (что изменилось)
        let delta = calculate_chunk_delta(&original_chunk, chunk_data, world);

        // Сохранять только если есть изменения
        if !delta.is_empty() {
            chunk_deltas.insert(*coord, delta);
        }
    }

    Ok(SaveFile {
        version: 1,
        world_seed,
        player: save_player(world)?,
        chunk_deltas,
        meta: SaveMetadata {
            playtime: world.resource::<Time>().elapsed_seconds_f64(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    })
}

/// Вычислить delta между procgen и текущим состоянием
fn calculate_chunk_delta(
    original: &ChunkData,
    current: &ChunkData,
    world: &World,
) -> ChunkDelta {
    let mut delta = ChunkDelta {
        removed_entities: HashSet::new(),
        added_entities: Vec::new(),
        modified_entities: Vec::new(),
        removed_items: HashSet::new(),
    };

    // TODO: Implement delta calculation
    // Сравнить original.procgen_params.enemy_count с current.entities.len()
    // Найти entities которые были удалены (despawned)
    // Найти entities которые были добавлены (не в procgen)
    // Найти entities с изменённым Health/AIState

    delta
}
```

### Load процесс

```rust
// === voidrun_simulation/src/save/load.rs ===

pub fn load_game(save: SaveFile, commands: &mut Commands) {
    // === 1. Установить world seed ===
    commands.insert_resource(WorldSeed(save.world_seed));

    // === 2. Spawn игрока ===
    commands.spawn((
        Player,
        save.player.position,
        Health { current: save.player.health, max: 100.0 },
        Stamina { current: save.player.stamina, max: 100.0 },
        Inventory { items: save.player.inventory },
        // ...
    ));

    // === 3. Сохранить chunk deltas (будут применены при генерации) ===
    commands.insert_resource(SavedChunkDeltas(save.chunk_deltas));

    // === 4. update_chunk_loading запустится и загрузит чанки вокруг игрока ===
    // При генерации чанка apply_chunk_delta будет вызван автоматически
}

/// При генерации чанка применить delta из save
fn generate_chunk_with_delta(
    coord: ChunkCoord,
    world_seed: u64,
    deltas: &SavedChunkDeltas,
) -> ChunkData {
    // === 1. Базовая procgen ===
    let mut chunk = generate_chunk(coord, world_seed);

    // === 2. Применить delta (если есть) ===
    if let Some(delta) = deltas.0.get(&coord) {
        // Убрать entities которые были удалены игроком
        // NOTE: Entities ещё не spawned, нужно пометить в procgen_params

        chunk.procgen_params.apply_delta(delta);
    }

    chunk
}
```

### Размер save файла

**Пример сценария:**
- World seed: 8 bytes
- Player: ~200 bytes
- Исследовано 100 чанков, изменения в 20 из них:
  - Chunk delta (1 убитый NPC, 2 собранных item): ~50 bytes
  - 20 чанков × 50 bytes = 1000 bytes

**Итого:** 8 + 200 + 1000 = **~1.2 KB** для нескольких часов игры.

**Сравнение:**
- **Full snapshot** (все entities): 1000 entities × 100 bytes = 100 KB
- **Seed + deltas**: ~1-5 KB

**Компрессия:** При необходимости можно сжать (gzip, zstd) → **~500 bytes**.

## Обоснование

### Почему chunk-based

**Альтернативы:**
- **Full world generation** — весь мир сразу (невозможно для infinite world)
- **Room-based** — каждая комната = отдельный уровень (не seamless)
- **Octree/quadtree** — adaptive subdivision (сложнее реализовать)

**Chunk-based преимущества:**
- ✅ Proven approach (Minecraft, No Man's Sky, Astroneer)
- ✅ Простота (понятная grid система)
- ✅ Streaming (загружаем только видимое)
- ✅ Determinism (chunk = единица procgen)
- ✅ MMO-ready (каждый игрок = load radius, объединяем)

### Почему 32x32 метра

**Обоснование:**
- ~10-15 секунд traversal (оптимальная granularity)
- 1-2 комнаты (логическая единица для станции)
- ~5-10 entities (manageable complexity)
- Not too small (не слишком много загрузок)
- Not too big (не слишком долгая генерация)

**Configurable:** Можно менять в `ChunkSize` resource.

### Почему seed + deltas

**Альтернативы:**
- **Full snapshot** — сохранить все entities (100+ KB save files)
- **Только seed** — не сохраняет изменения (игрок убил врага → respawn после Load)

**Seed + deltas преимущества:**
- ✅ **Tiny saves** — 1-5 KB вместо 100 KB
- ✅ **Infinite world** — не храним незатронутые чанки
- ✅ **Determinism** — procgen воспроизводима
- ✅ **Player agency preserved** — убитые враги не respawn, построенные базы сохранены

**Trade-off:**
- 🟡 Delta calculation overhead при save (~10-50ms для 20 чанков)
- 🟡 Procgen должна быть детерминированной (требует тестов)

## Влияния

### Новые компоненты/ресурсы

**voidrun_simulation:**
```rust
#[derive(Resource)]
pub struct LoadedChunks { chunks: HashMap<ChunkCoord, ChunkData>, load_radius: i32 }

#[derive(Resource)]
pub struct WorldSeed(pub u64);

#[derive(Component)]
pub struct ParentChunk(pub ChunkCoord);

#[derive(Resource)]
pub struct ChunkLoadSettings { load_radius: i32, unload_delay: f32 }

#[derive(Resource)]
pub struct SavedChunkDeltas(HashMap<ChunkCoord, ChunkDelta>);
```

**voidrun_godot:**
```rust
#[derive(Resource)]
pub struct ChunkGeometry { loaded: HashMap<ChunkCoord, Gd<Node3D>> }

#[derive(Resource)]
pub struct ZoneRegistry { nav_regions: HashMap<ChunkCoord, Gd<NavigationRegion3D>> }
```

### Новые события

```rust
#[derive(Event)]
pub enum ChunkEvent {
    Load { coord: ChunkCoord, biome: BiomeType },
    Unload { coord: ChunkCoord },
}
```

### Новые системы

**voidrun_simulation:**
- `update_chunk_loading` — core chunk management
- `generate_chunk` — procgen
- `save_game`, `load_game` — persistence

**voidrun_godot:**
- `process_chunk_events` — геометрия loading/unloading
- `spawn_entities_in_loaded_chunks` — NPC spawn на NavMesh

### Зависимости

**Cargo.toml:**
```toml
[dependencies]
rand = "0.8"
noise = "0.9"  # Perlin noise для биомов
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"  # Binary serialization для saves
```

### Тесты

**Детерминизм:**
```rust
#[test]
fn test_chunk_generation_deterministic() {
    let seed = 12345;
    let coord = ChunkCoord::new(5, 10);

    let chunk1 = generate_chunk(coord, seed);
    let chunk2 = generate_chunk(coord, seed);

    assert_eq!(chunk1.biome, chunk2.biome);
    assert_eq!(chunk1.procgen_params, chunk2.procgen_params);
}
```

**Streaming:**
```rust
#[test]
fn test_chunk_loading_unloading() {
    let mut app = App::new();
    app.add_systems(Update, update_chunk_loading);

    // Spawn игрока
    app.world.spawn((Player, StrategicPosition { chunk: (0, 0), local_offset: Vec2::ZERO }));

    // Tick
    app.update();

    let loaded = app.world.resource::<LoadedChunks>();
    assert_eq!(loaded.chunks.len(), 9); // 3x3 с radius=1

    // Переместить игрока
    // ... assert что старые чанки выгрузились
}
```

**Save/Load:**
```rust
#[test]
fn test_save_load_roundtrip() {
    let mut app = App::new();

    // Setup world
    app.insert_resource(WorldSeed(12345));
    // ... spawn player, explore chunks, kill enemies

    // Save
    let save = save_game(&app.world).unwrap();
    assert!(save.chunk_deltas.len() > 0);

    // Load в новый world
    let mut app2 = App::new();
    load_game(save, &mut app2.world);

    app2.update();

    // Verify player position, killed enemies не respawn
}
```

## Риски и митигация

### Риск 1: Procgen не детерминирован

**Описание:** Один и тот же seed → разный контент (из-за float precision, thread races).

**Вероятность:** Средняя (если использовать system random или thread RNG)

**Влияние:** Критическое (save/load broken)

**Митигация:**
- Использовать `StdRng::seed_from_u64()` (детерминированный)
- Фиксированный порядок генерации (не зависит от execution order)
- Property tests: `generate_chunk(coord, seed)` × 1000 раз → всегда одинаковый результат

**Метрики:**
- Determinism test pass rate = 100% (критично)

### Риск 2: Слишком много chunk transitions

**Описание:** Игрок быстро бегает → частые load/unload → FPS drops.

**Вероятность:** Низкая (с hysteresis unload_delay)

**Влияние:** Среднее (stuttering)

**Митигация:**
- Unload hysteresis (5 секунд задержка перед выгрузкой)
- Async loading (chunk geometry в отдельном thread)
- Preload соседних чанков (load_radius + 1 для предзагрузки)

**Метрики:**
- Chunk load time < 50ms (OK)
- Chunk load time > 200ms (проблема — оптимизировать procgen)

### Риск 3: Delta calculation overhead

**Описание:** Save занимает слишком долго (вычисление diffs для 100+ чанков).

**Вероятность:** Низкая (большинство чанков без изменений)

**Влияние:** Среднее (lag spike при save)

**Митигация:**
- Incremental delta tracking (не пересчитывать каждый save)
- Async save (в отдельном thread)
- Dirty flag (пересчитывать только изменённые чанки)

**Метрики:**
- Save time < 100ms (OK)
- Save time > 1000ms (проблема — async save)

### Риск 4: NavMesh baking задержка

**Описание:** Chunk load → NavMesh bake → NPC spawn delay → видимая задержка.

**Вероятность:** Средняя (в Godot 4.x baking может быть медленным)

**Влияние:** Низкое (визуальная задержка ~0.5-2 секунды)

**Митигация:**
- Pre-baked NavMesh в prefabs (не runtime baking)
- Async baking (Godot NavigationServer3D поддерживает)
- Spawn NPC после bake complete (не сразу)

**Метрики:**
- Chunk load → NPC spawn < 1 секунда (OK)

## Альтернативы (отклонены)

### Full world snapshot saves

```rust
struct SaveFile {
    entities: Vec<EntitySnapshot>, // ВСЕ entities
}
```

**Почему отклонено:**
- 100+ KB save files (vs 1-5 KB с deltas)
- Infinite world невозможен (миллионы entities)

### Room-based без chunks

**Почему отклонено:**
- Не seamless (loading screens между комнатами)
- Сложнее MMO (комната = единица lock для multiplayer?)

### Octree/Quadtree adaptive subdivision

**Почему отклонено:**
- Over-engineering (chunk grid проще)
- Сложнее debug (irregular boundaries)

## План имплементации

### Фаза 1: Chunk System Core (2-3 дня)

1. `ChunkCoord`, `ChunkData`, `LoadedChunks` types
2. `update_chunk_loading` система (load/unload logic)
3. Простейшая procgen (один биом, фиксированное количество врагов)
4. `ChunkEvent::Load/Unload` события
5. Unit tests (детерминизм, loading/unloading)

### Фаза 2: Godot Integration (1-2 дня)

6. `process_chunk_events` система (geometry loading)
7. `spawn_entities_in_loaded_chunks` (NPC spawn на NavMesh)
8. Chunk prefabs (corridor, warehouse scenes)
9. Integration test: игрок ходит, чанки грузятся/выгружаются

### Фаза 3: Procgen Content (2-3 дня)

10. Биомы (5-7 типов комнат)
11. Perlin noise для biome distribution
12. Детерминированная генерация врагов/лута (RNG per chunk)
13. Property tests (1000 генераций → всегда одинаково)

### Фаза 4: Save/Load (1-2 дня)

14. `SaveFile` структура
15. `calculate_chunk_delta` (diff алгоритм)
16. `save_game`, `load_game` функции
17. Integration test: Save → Load → verify state

**Итого:** 6-10 дней

## MMO Scaling (будущее)

Архитектура **уже готова** для MMO:

### Server-side

```rust
// Объединить load radius всех игроков
fn update_chunk_loading_multiplayer(
    players: Query<&StrategicPosition, With<Player>>,
    mut loaded_chunks: ResMut<LoadedChunks>,
) {
    let mut required_chunks = HashSet::new();

    // Для КАЖДОГО игрока
    for player_pos in players.iter() {
        // Добавить чанки вокруг этого игрока
        for dx in -LOAD_RADIUS..=LOAD_RADIUS {
            for dy in -LOAD_RADIUS..=LOAD_RADIUS {
                required_chunks.insert(player_pos.chunk + IVec2::new(dx, dy));
            }
        }
    }

    // Load/unload как обычно
    // ...
}
```

**Scaling:**
- 10 игроков рядом → 9 чанков (shared)
- 10 игроков в разных местах → ~90 чанков (но большинство unshared → можно offload на другие servers)

### Client-side

**Клиент получает:**
- `ChunkEvent::Load/Unload` от сервера
- State sync только для entities в видимых чанках

**Network traffic:**
- Chunk load: `ChunkEvent::Load { coord, biome }` = 12 bytes
- Entity spawn: `StrategicPosition` = 8 bytes (не полный Transform!)

## Заключение

**Chunk-based streaming world** = proven, simple, scalable решение для процедурной генерации infinite world.

**Ключевые достижения:**
- ✅ Infinite world (Minecraft-style)
- ✅ Детерминизм (seed → consistent content)
- ✅ Tiny saves (1-5 KB seed + deltas)
- ✅ Streaming (подгружаем только видимое)
- ✅ MMO-ready (multi-player load radius)

**Trade-offs:**
- 🟡 Procgen должна быть детерминированной (требует дисциплины)
- 🟡 Grid-based (не adaptive subdivision)
- 🟡 Save/Load = chunk granularity (не entity granularity)

**Следующие шаги:** См. План имплементации (Фаза 1-4, 6-10 дней).

---

**См. также:**
- [ADR-005: Transform Ownership & Strategic Positioning](ADR-005-transform-ownership-strategic-positioning.md) — StrategicPosition component usage
- [ADR-004: Command/Event Architecture](ADR-004-command-event-architecture.md) — ChunkEvent sync ECS↔Godot
