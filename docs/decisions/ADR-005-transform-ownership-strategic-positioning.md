# ADR-005: Transform Ownership & Strategic Positioning

**Дата:** 2025-01-10
**Статус:** ✅ ПРИНЯТО
**Связанные ADR:** [ADR-003](ADR-003-ecs-vs-godot-physics-ownership.md), [ADR-006](ADR-006-chunk-based-streaming-world.md)

## Контекст

**ADR-003 (Hybrid Architecture)** определил разделение ответственности:
- **ECS (Strategic Layer):** Authoritative game state, AI decisions, combat rules
- **Godot (Tactical Layer):** Transform, physics, animations

Необходимо решить:

1. **Кто владеет Transform** (position, rotation)?
2. **Как ECS отслеживает позицию** для AI/квестов/экономики?
3. **Как работает sync** между ECS и Godot?

### Критическое требование: Процедурная генерация

Из разговора с пользователем:

> "для процедурной генерации в будущем важно учесть этот факт. у меня нет ресурсов чтобы делать целые уровни и расставлять там врагов - все должна решать процедурка"

**Следствия:**
- Уровни генерируются в runtime (не в редакторе)
- NavigationMesh строится динамически (Godot)
- Spawn positions определяются NavMesh (ECS не знает геометрию)
- ECS **не может** диктовать точные координаты (X, Y, Z)

## Решение

**Godot owns Transform (authoritative), ECS owns StrategicPosition (zone-based)**

### Разделение ответственности

```
┌─────────────────────────────────────────┐
│ Godot (Tactical Layer)                  │
│ OWNS: Transform (authoritative)         │
│                                         │
│ - Position (X, Y, Z) — точные координаты│
│ - Rotation (Quat)                       │
│ - Physics (CharacterBody3D)             │
│ - NavigationAgent3D (pathfinding)       │
│                                         │
│ Определяет WHERE entity находится       │
└─────────────────────────────────────────┘
         ↑ reads (редко)    ↓ commands
┌─────────────────────────────────────────┐
│ ECS (Strategic Layer)                   │
│ OWNS: StrategicPosition (authoritative) │
│                                         │
│ - Chunk (IVec2) — зона/регион           │
│ - LocalOffset (Vec2) — для respawn      │
│ - Game state (Health, AI, combat)       │
│                                         │
│ Определяет WHAT entity делает           │
└─────────────────────────────────────────┘
```

**Ключевое отличие:**
- **Godot Transform** = tactical position (точная позиция для physics/rendering)
- **ECS StrategicPosition** = strategic position (zone/chunk для AI/quests/economy)

### StrategicPosition Component

```rust
// === voidrun_simulation/src/components.rs ===

/// Стратегическая позиция entity (chunk-based)
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategicPosition {
    /// Chunk координаты (IVec2 в бесконечной сетке)
    pub chunk: ChunkCoord,

    /// Локальный offset внутри чанка (0..CHUNK_SIZE метров)
    /// Используется для deterministic respawn при Load
    pub local_offset: Vec2,
}

pub type ChunkCoord = IVec2; // (x, y)

impl StrategicPosition {
    /// Конвертировать в приблизительные мировые координаты
    /// (для визуализации, не для точных вычислений)
    pub fn to_world_position(&self, chunk_size: f32) -> Vec3 {
        Vec3::new(
            self.chunk.x as f32 * chunk_size + self.local_offset.x,
            0.0, // Y определяется Godot NavMesh
            self.chunk.y as f32 * chunk_size + self.local_offset.y,
        )
    }

    /// Создать из мировых координат (для обновления из Godot)
    pub fn from_world_position(world_pos: Vec3, chunk_size: f32) -> Self {
        let chunk_x = (world_pos.x / chunk_size).floor() as i32;
        let chunk_z = (world_pos.z / chunk_size).floor() as i32;

        Self {
            chunk: ChunkCoord::new(chunk_x, chunk_z),
            local_offset: Vec2::new(
                world_pos.x.rem_euclid(chunk_size),
                world_pos.z.rem_euclid(chunk_size),
            ),
        }
    }
}
```

**Почему chunk-based:**
- **Компактность:** 8 bytes (2x i32) vs Transform 28 bytes (3x Vec3 + Quat)
- **Детерминизм:** Integer coords не зависят от floating point precision
- **AI logic:** AI думает зонами ("перейти в склад"), не точными координатами
- **Save/Load:** Сохраняем chunk ID, Godot восстанавливает Transform из NavMesh
- **Network:** 8 bytes sync vs 28 bytes Transform

## Sync механизм

### ECS → Godot (Commands, редко)

**ECS выдаёт HIGH-LEVEL команды**, Godot их исполняет:

```rust
// === voidrun_simulation/src/ai/systems.rs ===

fn ai_movement_decision(
    query: Query<(Entity, &AIState, &StrategicPosition)>,
    mut commands: EventWriter<AICommand>,
) {
    for (entity, state, current_pos) in query.iter() {
        match state {
            AIState::Chasing { target } => {
                // Узнать в какой зоне цель
                let target_zone = get_strategic_position(*target).chunk;

                // Если в другой зоне → команда перейти
                if target_zone != current_pos.chunk {
                    commands.send(AICommand::MoveToZone {
                        entity,
                        target_zone,
                    });
                }
            }
            _ => {}
        }
    }
}
```

**Godot система исполняет команду:**

```rust
// === voidrun_godot/src/ai_executor.rs ===

fn execute_ai_movement(
    mut commands: EventReader<AICommand>,
    visuals: Res<VisualRegistry>, // HashMap<Entity, Gd<CharacterBody3D>>
    zones: Res<ZoneRegistry>, // HashMap<ChunkCoord, Gd<NavigationRegion3D>>
) {
    for cmd in commands.read() {
        match cmd {
            AICommand::MoveToZone { entity, target_zone } => {
                // Получить Godot CharacterBody3D
                let character = visuals.get(*entity).unwrap();
                let mut nav_agent = character.get_node_as::<NavigationAgent3D>("NavigationAgent3D");

                // Найти точку входа в целевую зону
                let zone_entrance = find_zone_entrance(*target_zone, &zones);

                // NavigationAgent3D автоматически рассчитает путь
                nav_agent.set_target_position(zone_entrance);

                // Godot physics loop будет двигать персонажа
            }
            _ => {}
        }
    }
}
```

**Частота:** По необходимости (event-driven), обычно <1 Hz.

### Godot → ECS (Events, редко)

#### Zone Transitions

**Godot отправляет события когда entity пересекает границу чанка:**

```rust
// === voidrun_godot/src/zone_tracker.rs ===

fn track_zone_transitions(
    query: Query<(Entity, &StrategicPosition)>,
    visuals: Res<VisualRegistry>,
    mut events: EventWriter<GodotTransformEvent>,
    chunk_size: Res<ChunkSize>,
) {
    for (entity, strategic_pos) in query.iter() {
        // Получить реальную позицию из Godot (authoritative)
        if let Some(character) = visuals.get(entity) {
            let actual_position = character.get_position();

            // Определить chunk из реальной позиции
            let detected_chunk = ChunkCoord::new(
                (actual_position.x / chunk_size.0).floor() as i32,
                (actual_position.z / chunk_size.0).floor() as i32,
            );

            // Если чанк изменился → событие
            if detected_chunk != strategic_pos.chunk {
                events.send(GodotTransformEvent::ZoneTransition {
                    entity,
                    new_chunk: detected_chunk,
                });
            }
        }
    }
}
```

**ECS обновляет StrategicPosition:**

```rust
// === voidrun_simulation/src/systems/position_sync.rs ===

fn update_strategic_position(
    mut events: EventReader<GodotTransformEvent>,
    mut query: Query<&mut StrategicPosition>,
    mut zone_events: EventWriter<ZoneTransitionEvent>,
) {
    for event in events.read() {
        if let GodotTransformEvent::ZoneTransition { entity, new_chunk } = event {
            if let Ok(mut pos) = query.get_mut(*entity) {
                let old_chunk = pos.chunk;
                pos.chunk = *new_chunk;

                // Событие для других систем (квесты, экономика)
                zone_events.send(ZoneTransitionEvent {
                    entity: *entity,
                    from: old_chunk,
                    to: *new_chunk,
                });
            }
        }
    }
}
```

**Частота:** 0.1-1 Hz (раз в секунду проверка, не каждый frame).

#### PostSpawn Position Correction

**НОВОЕ:** Godot отправляет точную позицию после spawn для детерминистичных saves.

```rust
// === voidrun_godot/src/spawn_system.rs ===

fn spawn_entities_in_loaded_chunks(
    query: Query<(Entity, &StrategicPosition, &VisualPrefab), Added<StrategicPosition>>,
    zones: Res<ZoneRegistry>,
    mut visuals: ResMut<VisualRegistry>,
    mut events: EventWriter<GodotTransformEvent>,
) {
    for (entity, strategic_pos, prefab) in query.iter() {
        let nav_region = match zones.get(&strategic_pos.chunk) {
            Some(region) => region,
            None => continue,
        };

        // Найти spawn point на NavMesh (используем local_offset как hint)
        let hint_position = Vector3::new(
            strategic_pos.local_offset.x,
            0.0,
            strategic_pos.local_offset.y,
        );
        let spawn_position = find_nearest_navmesh_point(nav_region, hint_position);

        let scene = load::<PackedScene>(&prefab.path);
        let mut instance = scene.instantiate_as::<CharacterBody3D>();
        instance.set_position(spawn_position);
        instance.set_meta("entity_id".into(), entity.index().to_variant());

        nav_region.add_child(instance.clone());
        visuals.insert(entity, instance);

        // НОВОЕ: отправить точную позицию обратно в ECS
        events.send(GodotTransformEvent::PostSpawn {
            entity,
            actual_position: Vec3::new(
                spawn_position.x,
                spawn_position.y,
                spawn_position.z,
            ),
        });
    }
}
```

**ECS корректирует local_offset после spawn:**

```rust
// === voidrun_simulation/src/systems/position_sync.rs ===

fn update_local_offset_after_spawn(
    mut events: EventReader<GodotTransformEvent>,
    mut query: Query<&mut StrategicPosition>,
    chunk_size: Res<ChunkSize>,
) {
    for event in events.read() {
        if let GodotTransformEvent::PostSpawn { entity, actual_position } = event {
            if let Ok(mut pos) = query.get_mut(*entity) {
                // Корректируем local_offset точными координатами NavMesh
                pos.local_offset = Vec2::new(
                    actual_position.x.rem_euclid(chunk_size.0),
                    actual_position.z.rem_euclid(chunk_size.0),
                );

                // Теперь Save сохранит ТОЧНЫЕ координаты!
            }
        }
    }
}
```

**Результат:**
- Save хранит **exact** local_offset (откорректированный NavMesh)
- Load спавнит entity на **точно том же месте** (hint = exact offset)
- Детерминизм ✅ (нет ±2 метра дрифта)

### Change Detection для визуальной синхронизации

**Godot системы используют Changed<T> для sync:**

```rust
// === voidrun_godot/src/health_bar_sync.rs ===

fn sync_health_bars(
    // Только entity где Health изменился
    query: Query<(Entity, &Health), Changed<Health>>,
    visuals: Res<VisualRegistry>,
) {
    for (entity, health) in query.iter() {
        if let Some(character) = visuals.get(entity) {
            let mut health_bar = character.get_node_as::<ProgressBar>("HealthBar");
            health_bar.set_value((health.current / health.max * 100.0) as f64);
        }
    }
}
```

**Частота:** Каждый frame (но только изменённые entity благодаря Changed<T>).

## Procgen Workflow

### 1. ECS генерирует chunk content (логика)

```rust
fn spawn_enemies_in_chunk(
    chunk: &ChunkData,
    commands: &mut Commands,
    rng: &mut StdRng,
) {
    let enemy_count = match chunk.biome {
        BiomeType::Warehouse => rng.gen_range(3..8),
        BiomeType::Corridor => rng.gen_range(1..3),
        _ => 0,
    };

    for _ in 0..enemy_count {
        commands.spawn((
            // Стратегическая позиция: chunk + random offset
            StrategicPosition {
                chunk: chunk.coord,
                local_offset: Vec2::new(
                    rng.gen_range(0.0..CHUNK_SIZE),
                    rng.gen_range(0.0..CHUNK_SIZE),
                ),
            },

            Health { current: 100.0, max: 100.0 },
            AIState::Idle,
            VisualPrefab { path: "res://prefabs/enemy_grunt.tscn".into() },

            ParentChunk(chunk.coord), // Маркер принадлежности
        ));
    }
}
```

**ECS НЕ знает** точные координаты (X, Y, Z) — только chunk + hint (local_offset).

### 2. Godot загружает chunk geometry

```rust
fn process_chunk_load_events(
    mut events: EventReader<ChunkEvent>,
    mut geometry: ResMut<ChunkGeometry>,
    scene_root: Res<GodotSceneRoot>,
    mut zones: ResMut<ZoneRegistry>,
) {
    for event in events.read() {
        if let ChunkEvent::Load { coord, biome } = event {
            // Выбрать prefab по биому
            let prefab_path = match biome {
                BiomeType::Warehouse => "res://chunks/warehouse_chunk.tscn",
                BiomeType::Corridor => "res://chunks/corridor_chunk.tscn",
                _ => "res://chunks/default_chunk.tscn",
            };

            // Загрузить Godot scene
            let scene = load::<PackedScene>(prefab_path);
            let mut chunk_node = scene.instantiate_as::<Node3D>();

            // Позиция чанка в мире
            const CHUNK_SIZE: f32 = 32.0;
            chunk_node.set_position(Vector3::new(
                coord.x as f32 * CHUNK_SIZE,
                0.0,
                coord.y as f32 * CHUNK_SIZE,
            ));

            scene_root.add_child(chunk_node.clone());

            // Зарегистрировать NavigationRegion для спавна NPC
            if let Some(nav_region) = chunk_node.try_get_node_as::<NavigationRegion3D>("NavigationRegion3D") {
                zones.insert(*coord, nav_region);
            }

            geometry.loaded.insert(*coord, chunk_node);
        }
    }
}
```

### 3. Godot спавнит NPC на NavMesh

```rust
fn spawn_entities_in_loaded_chunks(
    // Added<StrategicPosition> = только что заспавнены в ECS
    query: Query<(Entity, &StrategicPosition, &VisualPrefab), Added<StrategicPosition>>,
    zones: Res<ZoneRegistry>,
    mut visuals: ResMut<VisualRegistry>,
) {
    for (entity, strategic_pos, prefab) in query.iter() {
        // Получить NavigationRegion для чанка
        let nav_region = match zones.get(&strategic_pos.chunk) {
            Some(region) => region,
            None => continue, // Чанк ещё не загружен в Godot
        };

        // Найти spawn point на NavMesh (используем local_offset как hint)
        let hint_position = Vector3::new(
            strategic_pos.local_offset.x,
            0.0,
            strategic_pos.local_offset.y,
        );
        let spawn_position = find_nearest_navmesh_point(nav_region, hint_position);

        // Загрузить visual prefab
        let scene = load::<PackedScene>(&prefab.path);
        let mut instance = scene.instantiate_as::<CharacterBody3D>();

        // GODOT решает координаты (authoritative)
        instance.set_position(spawn_position);
        instance.set_meta("entity_id".into(), entity.index().to_variant());

        nav_region.add_child(instance.clone());
        visuals.insert(entity, instance);
    }
}

fn find_nearest_navmesh_point(nav_region: &Gd<NavigationRegion3D>, hint: Vector3) -> Vector3 {
    let nav_map = nav_region.get_navigation_map();
    NavigationServer3D::singleton().map_get_closest_point(nav_map, hint)
}
```

**Ключевое:** Godot **корректирует** local_offset чтобы найти валидную точку на NavMesh.

### 4. Transform остаётся в Godot

```rust
// === voidrun_godot/src/physics_loop.rs ===

// Godot CharacterBody3D._physics_process()
fn godot_physics_process(delta: f64) {
    // NavigationAgent3D управляет velocity
    let next_position = nav_agent.get_next_path_position();
    let direction = (next_position - global_position).normalized();

    velocity = direction * speed;

    // CharacterBody3D.move_and_slide()
    move_and_slide();

    // Transform обновляется автоматически
    // ECS НЕ знает об этом (пока entity не пересечёт границу чанка)
}
```

**ECS не синхронизирует Transform каждый frame** — только при zone transitions (~1 Hz).

## Save/Load

### Сохранение (компактно)

```rust
#[derive(Serialize, Deserialize)]
struct SavedEntity {
    id: u32,
    strategic_position: StrategicPosition, // 8 bytes
    health: f32,
    ai_state: AIState,
    // НЕ сохраняем Transform (Godot восстановит)
}
```

**Размер:** ~50 bytes на entity (vs ~200 bytes если хранить Transform + velocity + rotation).

### Загрузка

```rust
fn load_entity(saved: SavedEntity, commands: &mut Commands) {
    commands.spawn((
        StrategicPosition {
            chunk: saved.strategic_position.chunk,
            local_offset: saved.strategic_position.local_offset,
        },
        Health { current: saved.health, max: 100.0 },
        AIState::from(saved.ai_state),
        VisualPrefab { path: "res://prefabs/npc.tscn".into() },
    ));

    // Godot система (spawn_entities_in_loaded_chunks) увидит Added<StrategicPosition>
    // → Найдёт NavMesh точку рядом с local_offset
    // → Instantiate визуал
    // → Transform восстановлен!
}
```

**Гарантия:** Entity окажется **в том же чанке**, на **валидной NavMesh точке** (возможно чуть смещённой от original position, но это OK).

## Обоснование

### Почему Godot owns Transform

**Технические причины:**

1. **Процедурная генерация требует NavMesh**
   - ECS не знает геометрию уровня
   - Godot NavigationMesh определяет где можно spawn
   - Spawn position = `NavigationServer3D::map_get_closest_point()`

2. **Godot NavigationAgent3D = best pathfinding из коробки**
   - A* с obstacle avoidance
   - Не нужно дублировать в Rust
   - Проверено в production (множество Godot игр)

3. **Physics consistency**
   - Collisions, raycasts должны match visual position
   - Godot physics engine authoritative
   - Transform из ECS → desync с визуалом

**Геймплейные причины:**

4. **AI думает зонами, не координатами**
   - "Перейти в склад" (zone transition), не "идти к (123.45, 0, 67.89)"
   - Kenshi-style strategic movement
   - High-level goals → Godot исполняет детали

### Почему ECS owns StrategicPosition

**Технические причины:**

1. **Детерминизм**
   - Integer ChunkCoord не зависит от float precision
   - Одинаковый seed → одинаковые чанки
   - Transform (float) = недетерминирован (0.0001 разница накапливается)

2. **Компактность**
   - 8 bytes StrategicPosition vs 28 bytes Transform
   - Network sync: отправляем chunk ID (редко), не Transform (каждый frame)
   - Save files: ~50 bytes/entity vs ~200 bytes

3. **AI/Quest logic**
   - "Если игрок в зоне X → trigger квест"
   - "Если враг в зоне Y → alert nearby NPCs"
   - Zone-based triggers проще чем distance checks

**Геймплейные причины:**

4. **Kenshi-style world simulation**
   - Мир симулируется на уровне зон (не точных координат)
   - Караваны торговцев двигаются между городами (zones), не по пикселям
   - ECS симулирует "что происходит", Godot визуализирует "как выглядит"

### Trade-offs

**Преимущества:**

- ✅ **Procgen-friendly** — Godot может решать координаты
- ✅ **Simple AI** — high-level goals, не micromanagement
- ✅ **Компактные saves** — 8 bytes chunk vs 28 bytes Transform
- ✅ **Детерминизм** — integer coords stable
- ✅ **Network-friendly** — редкие zone transitions, не каждый frame Transform sync

**Недостатки:**

- 🟡 **Eventual consistency** — StrategicPosition обновляется с задержкой (0.1-1 Hz)
- 🟡 **Precision loss** — local_offset = hint, не exact position после Load
- 🟡 **Godot dependency** — ECS не может работать headless для точных координат

**Митигация недостатков:**

- Eventual consistency: OK для strategic gameplay (не нужна пиксель-точность для AI)
- Precision loss: OK для respawn (±1 метр не критично)
- Godot dependency: Headless режим возможен с mock NavMesh (для тестов экономики/квестов)

## Влияния

### Новые компоненты

**voidrun_simulation:**
```rust
#[derive(Component)]
pub struct StrategicPosition { pub chunk: ChunkCoord, pub local_offset: Vec2 }

#[derive(Component)]
pub struct ParentChunk(pub ChunkCoord); // Маркер принадлежности к чанку

pub type ChunkCoord = IVec2;
```

### Удалённые компоненты

- ~~`Transform` в ECS~~ (остаётся только в Godot CharacterBody3D/Node3D)
- ~~`Velocity` в ECS~~ (Godot physics управляет)

### Изменённые системы

**AI systems:**
```rust
// БЫЛО: работа с Transform
fn ai_chase(query: Query<(&mut Transform, &AIState)>) { ... }

// СТАЛО: работа с StrategicPosition
fn ai_chase(query: Query<(&StrategicPosition, &AIState)>) {
    // Логика на уровне зон, не координат
}
```

**Spawn systems:**
```rust
// БЫЛО: ECS решает координаты
commands.spawn((Transform::from_xyz(10.0, 0.0, 15.0), ...));

// СТАЛО: ECS решает chunk, Godot решает координаты
commands.spawn((StrategicPosition { chunk: (0, 0), local_offset: (10, 15) }, ...));
```

### Новые события

**voidrun_simulation/src/events/transform.rs:**
```rust
#[derive(Event)]
pub enum GodotTransformEvent {
    ZoneTransition { entity: Entity, new_chunk: ChunkCoord },
    PostSpawn { entity: Entity, actual_position: Vec3 },
}

#[derive(Event)]
pub struct ZoneTransitionEvent {
    pub entity: Entity,
    pub from: ChunkCoord,
    pub to: ChunkCoord,
}
```

### Тесты

**Unit tests (без Godot):**
```rust
#[test]
fn test_strategic_position_conversion() {
    let pos = StrategicPosition {
        chunk: ChunkCoord::new(2, 3),
        local_offset: Vec2::new(15.0, 20.0),
    };

    let world_pos = pos.to_world_position(32.0);
    assert_eq!(world_pos, Vec3::new(79.0, 0.0, 116.0)); // (2*32+15, 0, 3*32+20)

    let reconstructed = StrategicPosition::from_world_position(world_pos, 32.0);
    assert_eq!(reconstructed, pos);
}
```

**Integration tests (с Godot):**
- Spawn entity → Godot находит NavMesh точку → Transform != hint (OK)
- Save → Load → entity в том же чанке (±1 метр)

## Риски и митигация

### Риск 1: Desync между StrategicPosition и Transform

**Описание:** Entity физически пересёк границу чанка, но ECS ещё не получил ZoneTransition событие.

**Вероятность:** Средняя (если Godot система track_zone_transitions не запускается часто)

**Влияние:** Низкое (AI принимает решения на outdated data на ~0.1-1 секунду)

**Митигация:**
- Запускать track_zone_transitions каждые 0.5-1 секунду (достаточно)
- AI логика учитывает "fog of war" (неточная информация = feature, не bug)
- Критичные checks (например collision) делаются в Godot (authoritative)

**Метрики:**
- Zone transition latency < 1 секунда (OK)
- Zone transition latency > 5 секунд (проблема — увеличить частоту tracking)

### Риск 2: Precision loss при Save/Load

**Описание:** После Load entity смещается на ±0.5-2 метра от original position.

**Вероятность:** 100% (это design decision)

**Влияние:** Низкое (для большинства gameplay ситуаций)

**Митигация:**
- Local offset = hint для NavMesh (старается spawn близко)
- Для критичных объектов (например quest items) можно сохранять exact Transform
- Player position: сохранять Transform отдельно (precision важна)

**Метрики:**
- NPC spawn precision ±2 метра (OK)
- Player spawn precision ±0.1 метра (нужно exact Transform в save)

### Риск 3: Headless режим для тестов

**Описание:** ECS не может работать без Godot (нет NavMesh для spawn).

**Вероятность:** 100% (это design decision)

**Влияние:** Среднее (сложнее тестировать экономику/квесты в CI)

**Митигация:**
- Mock NavMesh для headless тестов (всегда возвращает hint position)
- Большинство ECS логики (combat, AI decisions) не требует точных координат
- Integration tests запускать с Godot headless mode (есть в Godot 4.x)

## Альтернативы (отклонены)

### ECS owns Transform (authoritative)

**Почему отклонено:**
- Процедурная генерация невозможна (ECS не знает NavMesh)
- Придётся дублировать pathfinding в Rust
- Godot визуалы станут "dumb viewers" (сложнее интегрировать)

### Полная синхронизация Transform (каждый frame)

```rust
fn sync_transforms(
    query: Query<(Entity, &Transform), Changed<Transform>>,
    visuals: Res<VisualRegistry>,
) {
    for (entity, transform) in query.iter() {
        visuals.get(entity).set_position(transform.translation);
    }
}
```

**Почему отклонено:**
- Overhead: синхронизация для всех moving entities каждый frame
- Latency: изменения в ECS → 1 frame delay → Godot визуал
- Unnecessary: AI не нужна точность до пикселя

### Dual ownership (ECS и Godot оба хранят Transform)

**Почему отклонено:**
- Source of truth неясен (кто authoritative?)
- Sync conflicts (ECS изменил, Godot изменил — кто прав?)
- Сложность (нужен conflict resolution)

## План имплементации

### Фаза 1: StrategicPosition component (1-2 часа)

1. Создать `StrategicPosition` component
2. `ChunkCoord` type alias (IVec2)
3. `to_world_position()`, `from_world_position()` методы
4. Unit tests для conversion

### Фаза 2: Убрать Transform из ECS (1-2 часа)

5. Удалить `Transform` из actor spawn
6. AI системы переписать на `StrategicPosition`
7. Компиляция + исправление ошибок

### Фаза 3: Godot spawn system (2-3 часа)

8. `spawn_entities_in_loaded_chunks` система
9. `find_nearest_navmesh_point` helper
10. `Added<StrategicPosition>` query
11. Instantiate визуалов на NavMesh

### Фаза 4: Zone transition tracking (1-2 часа)

12. `track_zone_transitions` система (Godot)
13. `GodotInputEvent::ZoneTransition` event
14. `update_strategic_position` система (ECS)

### Фаза 5: Save/Load (1 час)

15. Обновить `SavedEntity` (StrategicPosition вместо Transform)
16. Load система с Added<StrategicPosition> trigger
17. Integration test: Save → Load → проверить chunk

**Итого:** 6-10 часов (~1 день)

## Откат

Если подход не зайдёт:

**План B: ECS owns Transform для статичных объектов**
- Разделить entities на "moving" (Godot Transform) и "static" (ECS Transform)
- Static entities (dropped items, buildings) = ECS authoritative
- Moving entities (NPC, player) = Godot authoritative

**План C: Полная синхронизация Transform**
- Вернуться к Transform в ECS
- Sync каждый frame (overhead, но проще понять)

**Критерии для отката:**
- Procgen невозможна (NavMesh не работает)
- Desync проблемы (zone transitions не работают надёжно)
- Performance проблемы (tracking overhead)

**Вероятность отката:** <5%

## AI Perception (Godot + ECS)

**Вопрос:** Кто решает "NPC увидел игрока"?

**Ответ:** Hybrid подход — Godot определяет КОГДА увидел, ECS решает ЧТО делать.

### Godot (Tactical) — Vision Detection

**Area3D для vision cone (Rust код через godot-rust):**

```rust
// === voidrun_godot/src/nodes/vision_cone.rs ===

use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct VisionCone {
    base: Base<Area3D>,
    observer_entity: Option<Entity>,
}

#[godot_api]
impl IArea3D for VisionCone {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            observer_entity: None,
        }
    }

    fn ready(&mut self) {
        // Подписаться на сигналы collision
        let callable_entered = self.base().callable("on_body_entered");
        self.base_mut().connect("body_entered".into(), callable_entered);

        let callable_exited = self.base().callable("on_body_exited");
        self.base_mut().connect("body_exited".into(), callable_exited);
    }
}

#[godot_api]
impl VisionCone {
    #[func]
    fn on_body_entered(&mut self, body: Gd<CharacterBody3D>) {
        // Проверка line-of-sight (raycast)
        if self.is_visible_to(&body) {
            let observer_id = self.base().get_parent()
                .unwrap()
                .get_meta("entity_id".into())
                .to::<u32>();

            let target_id = body.get_meta("entity_id".into()).to::<u32>();

            // Отправить событие в ECS (через систему ниже)
            self.base_mut().set_meta(
                "spotted_target".into(),
                target_id.to_variant()
            );
        }
    }

    #[func]
    fn on_body_exited(&mut self, body: Gd<CharacterBody3D>) {
        let observer_id = self.base().get_parent()
            .unwrap()
            .get_meta("entity_id".into())
            .to::<u32>();

        let target_id = body.get_meta("entity_id".into()).to::<u32>();

        self.base_mut().set_meta(
            "lost_target".into(),
            target_id.to_variant()
        );
    }

    fn is_visible_to(&self, target: &Gd<CharacterBody3D>) -> bool {
        // Raycast для проверки препятствий
        let space_state = self.base()
            .get_world_3d()
            .unwrap()
            .get_direct_space_state()
            .unwrap();

        let mut query = PhysicsRayQueryParameters3D::new();
        query.set_from(self.base().get_global_position());
        query.set_to(target.get_global_position());

        let result = space_state.intersect_ray(query);

        // Если raycast не попал в препятствие → видим цель
        result.is_empty()
    }
}
```

**Rust система слушает meta changes и отправляет события:**

```rust
// === voidrun_godot/src/ai/perception.rs ===

fn listen_vision_events(
    visuals: Res<VisualRegistry>,
    mut events: EventWriter<GodotAIEvent>,
) {
    for (entity, character) in visuals.iter() {
        // Получить vision cone node
        if let Some(vision) = character.try_get_node_as::<Area3D>("VisionCone") {
            // Проверить spotted_target meta
            if vision.has_meta("spotted_target".into()) {
                let target_id = vision.get_meta("spotted_target".into()).to::<u32>();

                events.send(GodotAIEvent::ActorSpotted {
                    observer: *entity,
                    target: Entity::from_raw(target_id),
                });

                // Очистить meta
                vision.remove_meta("spotted_target".into());
            }

            // Проверить lost_target meta
            if vision.has_meta("lost_target".into()) {
                let target_id = vision.get_meta("lost_target".into()).to::<u32>();

                events.send(GodotAIEvent::ActorLost {
                    observer: *entity,
                    target: Entity::from_raw(target_id),
                });

                vision.remove_meta("lost_target".into());
            }
        }
    }
}
```

### ECS (Strategic) — AI Decisions

**ECS решает ЧТО делать после spotted:**

```rust
// === voidrun_simulation/src/ai/perception.rs ===

fn handle_actor_spotted(
    mut events: EventReader<GodotAIEvent>,
    mut query: Query<(&mut AIState, &Faction)>,
    target_query: Query<&Faction>,
) {
    for event in events.read() {
        if let GodotAIEvent::ActorSpotted { observer, target } = event {
            let (mut ai_state, faction) = query.get_mut(*observer).unwrap();
            let target_faction = target_query.get(*target).unwrap();

            // ECS РЕШАЕТ: враг или друг?
            if faction.is_hostile_to(target_faction) {
                *ai_state = AIState::Chasing { target: *target };
            } else if faction.is_ally(target_faction) {
                *ai_state = AIState::Idle; // Ignore
            }
        }
    }
}

fn handle_actor_lost(
    mut events: EventReader<GodotAIEvent>,
    mut query: Query<&mut AIState>,
) {
    for event in events.read() {
        if let GodotAIEvent::ActorLost { observer, target } = event {
            if let Ok(mut ai_state) = query.get_mut(*observer) {
                // Если chase прервался → вернуться к патрулированию
                if matches!(*ai_state, AIState::Chasing { target: t } if t == *target) {
                    *ai_state = AIState::Patrolling;
                }
            }
        }
    }
}
```

**Разделение ответственности:**
- **Godot (tactical):**
  - Vision cone collisions (Area3D)
  - Line-of-sight raycasts
  - КОГДА target вошёл/вышел из cone

- **ECS (strategic):**
  - Faction relationships (враг/друг)
  - AI state transitions (idle → chase)
  - ЧТО делать после spotted

---

## Заключение

**StrategicPosition (ECS) + Transform (Godot)** = чистое разделение ответственности для Hybrid Architecture с процедурной генерацией.

**Ключевые принципы:**
- Godot = tactical layer (точные координаты, NavMesh, pathfinding, vision detection)
- ECS = strategic layer (зоны, AI goals, game state, faction logic)
- Sync редко (zone transitions ~1 Hz, не каждый frame)
- Saves компактны (8 bytes chunk vs 28 bytes Transform)
- **PostSpawn коррекция** — детерминистичные saves (exact position)

**Новые изменения (2025-01-10):**
1. **PostSpawn event** — Godot отправляет точную позицию после spawn → ECS корректирует local_offset
2. **AI Vision в Rust** — VisionCone node (godot-rust) + ECS perception системы

**Следующие шаги:** См. План имплементации (Фаза 1-5).

---

**См. также:**
- [ADR-003: ECS vs Godot Physics Ownership](ADR-003-ecs-vs-godot-physics-ownership.md) — Hybrid Architecture foundation
- [ADR-004: Command/Event Architecture](ADR-004-command-event-architecture.md) — Bevy Events для sync
- [ADR-006: Chunk-based Streaming World](ADR-006-chunk-based-streaming-world.md) — ChunkCoord использование в procgen
