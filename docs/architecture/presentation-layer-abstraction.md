# VOIDRUN: Presentation Layer Abstraction (Engine-Agnostic Design)

## Дата создания: 2025-10-07
## Последнее обновление: 2025-10-07 (аудит 2024-2025)
## Версия: 2.0
## Статус: Validated + Performance Optimizations

---

## 1. Цель: изоляция симуляции от конкретного движка

### Рекомендация

Создать слой абстракции **PresentationClient trait** между Bevy ECS симуляцией и любым визуальным движком (Godot, Bevy Renderer, Unreal, Unity). Вся коммуникация идёт через явный protocol (Rust traits + data structs), что позволяет заменить движок с минимальными изменениями в логике игры.

### Обоснование из индустрии (2024)

**Успешные примеры абстракций:**
- **bgfx** (C++): cross-platform graphics API abstraction, "Bring Your Own Engine" философия
- **wgpu** (Rust): абстракция над Vulkan/Metal/DX12/WebGPU — используется в Bevy
- **Bevy 0.6 решение**: убрали свою абстракцию, используют wgpu напрямую (wgpu = достаточный уровень абстракции)

**Rust 2024 улучшения для traits:**
- Improved `impl Trait` capture: автоматический capture lifetimes (меньше boilerplate)
- Async traits: нативная поддержка async fn в traits (критично для streaming контента)
- Better ergonomics: меньше explicit lifetime annotations

**Ключевой инсайт:**
> Абстракция должна быть на **правильном уровне** — не слишком низкоуровневая (API calls), не слишком высокоуровневая (game logic). Уровень: "презентационные команды" (spawn visual, update transform, play animation).

### Trade-offs

**За слой абстракции:**
- ✅ Портируемость: замена движка = новая impl trait
- ✅ Тестируемость: MockClient для headless тестов
- ✅ Ясность контракта: protocol явно документирован
- ✅ Изоляция: bug в презентации не ломает симуляцию

**Против:**
- ⚠️ Boilerplate: нужен отдельный crate + trait definitions
- ⚠️ Indirect calls: возможен overhead (но negligible при правильном дизайне)
- ⚠️ Maintenance: при добавлении фич нужно обновлять trait

---

## 2. Архитектура модульной системы

### Структура crates (Rust workspace)

```
voidrun/
├─ voidrun_core/           # ECS компоненты, события, типы (engine-agnostic)
├─ voidrun_simulation/     # Bevy ECS системы (физика, AI, экономика)
├─ voidrun_protocol/       # PresentationClient trait + data structures
├─ voidrun_godot_client/   # Godot реализация PresentationClient
├─ voidrun_bevy_client/    # Bevy Renderer реализация (альтернатива Godot)
├─ voidrun_mock_client/    # Mock для тестов (headless)
└─ voidrun_server/         # Dedicated server binary (uses MockClient)
```

**Зависимости (направление стрелок = depends on):**

```
voidrun_godot_client ──→ voidrun_protocol ──→ voidrun_core
voidrun_bevy_client  ──→ voidrun_protocol ──→ voidrun_core
voidrun_mock_client  ──→ voidrun_protocol ──→ voidrun_core
                            ↑
                            │
                    voidrun_simulation
```

**Ключевой принцип:** `voidrun_simulation` знает только про `voidrun_protocol` trait, не про конкретные реализации.

---

## 3. PresentationClient trait (core contract)

### Scope: что входит в абстракцию

**Включено (презентационные команды):**
- ✅ Visual sync: Transform updates, Spawn/Despawn визуальных объектов
- ✅ Animations: Trigger animation states, Play/Stop sounds
- ✅ VFX: Particle effects, Post-processing triggers (hit flash, explosion)
- ✅ UI: Update UI elements с данными (health bar, inventory list)
- ✅ Audio: Play 3D positioned sounds, Music transitions

**Исключено (остаётся в Bevy):**
- ❌ Геймплейная физика (в Bevy через bevy_rapier)
- ❌ Hit detection (в Bevy)
- ❌ AI логика (в Bevy)
- ❌ Экономика/квесты (в Bevy)

### Trait definition (текстовое описание)

**PresentationClient trait:**

**Визуальные объекты:**
- `spawn_visual(entity_id, prefab_path, initial_transform) -> Result<()>`
  - Создать визуальный объект (3D модель, анимации)
  - entity_id: StableId из Bevy (u64), не Bevy Entity
  - prefab_path: универсальный путь (e.g. "prefabs/ship_engine")

- `despawn_visual(entity_id) -> Result<()>`
  - Удалить визуальный объект

- `update_transform(entity_id, position, rotation) -> Result<()>`
  - Синхронизация Transform (каждый FixedUpdate tick)

- `update_transform_batch(updates: &[TransformUpdate]) -> Result<()>`
  - Batch update для производительности (отправить 100+ entities за раз)

**Анимации:**
- `play_animation(entity_id, animation_name, loop_mode, blend_time) -> Result<()>`
  - Trigger анимация (walk, attack, death)

- `stop_animation(entity_id, animation_name) -> Result<()>`

**VFX и временные эффекты:**
- `trigger_vfx(effect_type, position, parameters) -> Result<()>`
  - Spawn particles/effects (explosion, sparks, blood)
  - effect_type: enum (Explosion, MuzzleFlash, HitSparks)
  - parameters: VFXParameters struct (scale, color, duration)

**UI:**
- `update_ui_text(element_id, text) -> Result<()>`
  - Обновить текстовое поле (health: "100/150")

- `update_ui_list(element_id, items: &[UIListItem]) -> Result<()>`
  - Обновить список (inventory items, market prices)

- `show_notification(message, duration, notification_type) -> Result<()>`
  - Показать временное уведомление (quest completed, item picked up)

**Audio:**
- `play_sound_3d(sound_id, position, volume) -> Result<()>`
  - 3D positioned звук (выстрел, взрыв)

- `play_music(track_id, fade_duration) -> Result<()>`
  - Музыкальный трек (с crossfade)

- `stop_sound(sound_id) -> Result<()>`

**Камера (опционально, может быть в клиенте):**
- `set_camera_target(entity_id) -> Result<()>`
  - Привязать камеру к entity (follow player ship)

- `shake_camera(intensity, duration) -> Result<()>`
  - Shake эффект (при взрыве)

**Input (обратное направление: Client → Simulation):**
- Не входит в PresentationClient trait
- Отдельный trait: `InputSource`
- Клиент имплементирует, симуляция читает

---

## 4. Data structures (protocol types)

### Базовые типы (voidrun_protocol crate)

**EntityId:**
- Type alias: `type EntityId = u64`
- Stable ID (переживает save/load/network)
- Маппинг Entity ↔ EntityId в Bevy через Resource

**Transform3D:**
```
(Текстовое описание структуры)

struct Transform3D {
    position: Vec3,      // f32 или fixed-point (зависит от детерминизма требований)
    rotation: Quat,      // quaternion для rotation
    scale: Vec3          // опционально, по умолчанию (1, 1, 1)
}
```

**TransformUpdate (batch sync):**
```
struct TransformUpdate {
    entity_id: EntityId,
    transform: Transform3D,
}
```

**SpawnVisualCommand:**
```
struct SpawnVisualCommand {
    entity_id: EntityId,
    prefab_path: String,         // универсальный путь (движок резолвит)
    initial_transform: Transform3D,
    metadata: Option<VisualMetadata>  // доп параметры (tint color, LOD override)
}
```

**VFXParameters:**
```
struct VFXParameters {
    scale: f32,
    color: Option<Color>,        // RGB или RGBA
    duration: f32,               // seconds
    custom_params: HashMap<String, f32>  // extension point
}
```

**UIListItem (для inventory/market UI):**
```
struct UIListItem {
    item_id: String,             // "item_laser_cannon_mk2"
    display_name: String,        // "Laser Cannon Mk.II"
    icon_path: String,           // "icons/weapons/laser_cannon.png"
    quantity: Option<u32>,       // для stackable items
    metadata: HashMap<String, String>  // price, description, etc.
}
```

**AnimationCommand:**
```
struct AnimationCommand {
    entity_id: EntityId,
    animation_name: String,      // "walk", "attack", "death"
    loop_mode: AnimationLoop,    // Once, Loop, PingPong
    blend_time: f32,             // seconds для smooth transition
}

enum AnimationLoop {
    Once,
    Loop,
    PingPong
}
```

---

## 5. Bevy интеграция: как использовать trait

### Bevy система отправляет команды в PresentationClient

**Архитектура:**

**Resource в Bevy World:**
```
(Текстовое описание)

Resource PresentationClientHandle:
  - Хранит Box<dyn PresentationClient + Send + Sync>
  - Инициализируется при startup (выбор реализации: Godot, Bevy, Mock)
```

**Система синхронизации Transform:**
```
(Текстовое описание алгоритма)

Система: SyncTransformsSystem (в FixedUpdate, после physics)

Query: Changed<Transform> (только изменённые)

Для каждого entity:
  1. Получить StableId из компонента StableIdComponent
  2. Конвертировать Bevy Transform → protocol Transform3D
  3. Добавить в batch buffer: Vec<TransformUpdate>

В конце системы:
  - client.update_transform_batch(&batch_buffer)
  - Отправить все updates одним вызовом (эффективнее чем по одному)
```

**Система spawn визуальных объектов:**
```
(Текстовое описание)

Система: SpawnVisualsSystem (триггерится событием)

EventReader: SpawnEntityEvent { entity, prefab_path, transform }

Обработка:
  1. Создать Bevy Entity с компонентами (Transform, Velocity, Health, etc.)
  2. Добавить StableIdComponent { id: next_stable_id }
  3. client.spawn_visual(stable_id, prefab_path, transform)
  4. Сохранить mapping: stable_id ↔ Bevy Entity в Registry resource
```

**Система VFX (реакция на события):**
```
(Текстовое описание)

Система: TriggerVFXSystem

EventReader: ProjectileHit, ShipDestroyed, MeleeHit

При ProjectileHit:
  - client.trigger_vfx(VFXType::HitSparks, hit.position, params)
  - Опционально: client.play_sound_3d("hit_metal", hit.position, 1.0)

При ShipDestroyed:
  - client.trigger_vfx(VFXType::Explosion, ship.position, large_explosion_params)
  - client.play_sound_3d("explosion_large", ship.position, 1.5)
  - client.shake_camera(0.5, 1.0)
```

---

## 6. Реализации PresentationClient (примеры)

### GodotClient (через gdext)

**Архитектура:**
```
(Текстовое описание)

struct GodotClient {
    godot_root: Gd<Node>,              // Root node в Godot scene tree
    entity_nodes: HashMap<EntityId, Gd<Node3D>>,  // mapping entity → Godot Node
    prefab_cache: HashMap<String, Gd<PackedScene>>,  // кэш загруженных prefab'ов
}

Реализация spawn_visual:
  1. Проверить prefab_cache, если нет → загрузить через ResourceLoader
  2. Instantiate scene: prefab.instantiate_as::<Node3D>()
  3. Установить transform: node.set_global_transform(...)
  4. Добавить в scene tree: godot_root.add_child(node)
  5. Сохранить в entity_nodes: map[entity_id] = node

Реализация update_transform_batch:
  1. Для каждого TransformUpdate в batch:
     - Найти node в entity_nodes[entity_id]
     - node.set_global_position(update.position)
     - node.set_quaternion(update.rotation)
  2. Godot автоматически интерполирует между updates (если FPS > FixedUpdate rate)

Реализация trigger_vfx:
  1. Загрузить VFX prefab (e.g. "vfx/explosion.tscn")
  2. Instantiate + set position
  3. Запустить AnimationPlayer или GPUParticles3D
  4. Автоудаление через Timer (после duration)
```

---

### BevyClient (Bevy Renderer)

**Архитектура:**
```
(Текстовое описание)

struct BevyClient {
    bevy_app: App,                     // отдельный Bevy App для рендера
    entity_mapping: HashMap<EntityId, Entity>,  // protocol ID → Bevy render Entity
}

Важно: Два разных Bevy World:
  - Simulation World (в voidrun_simulation): логика, ECS
  - Render World (в BevyClient): только визуал

Реализация spawn_visual:
  1. Загрузить .gltf scene через Bevy AssetServer
  2. Spawn entity в Render World с компонентами:
     - SceneBundle { scene: gltf_handle, transform, ... }
  3. Сохранить mapping: protocol EntityId → Render Entity

Реализация update_transform_batch:
  1. Отправить TransformUpdate events в Render World
  2. Bevy система в Render World обрабатывает:
     - Query<&mut Transform> + filter по entity_mapping
     - Обновить Transform компоненты

Sync между Simulation и Render World:
  - Каждый frame: batch send updates через channel
  - Render World применяет в своём schedule
```

---

### MockClient (для headless тестов)

**Архитектура:**
```
(Текстовое описание)

struct MockClient {
    spawned_entities: HashSet<EntityId>,
    transform_log: Vec<TransformUpdate>,     // для проверки в тестах
    vfx_log: Vec<VFXTrigger>,
    animation_log: Vec<AnimationCommand>,
}

Реализация spawn_visual:
  - spawned_entities.insert(entity_id)
  - Ничего не рисует (headless)

Реализация update_transform_batch:
  - transform_log.extend(batch)
  - Для assertions в тестах

Использование в тестах:
  - Запустить симуляцию 1000 тиков
  - Проверить: mock_client.spawned_entities.contains(&expected_entity)
  - Проверить: mock_client.transform_log последний update имеет правильную позицию
  - Проверить детерминизм: два запуска дают одинаковый transform_log
```

---

## 7. Input abstraction (обратное направление)

### InputSource trait (Client → Simulation)

**Scope:** Клиент собирает input, отправляет в симуляцию

**Trait definition:**
```
(Текстовое описание)

trait InputSource {
    fn poll_commands(&mut self) -> Vec<InputCommand>;
    fn is_key_pressed(&self, key: KeyCode) -> bool;       // для simple checks
    fn mouse_position(&self) -> Option<Vec2>;
    fn mouse_delta(&self) -> Vec2;
}

enum InputCommand {
    MoveForward,
    MoveBackward,
    StrafeLeft,
    StrafeRight,
    Jump,
    Fire { aim_direction: Vec3 },
    UseItem { item_id: String },
    Interact { target_entity: EntityId },
}
```

**Bevy система читает input:**
```
(Текстовое описание)

Система: ProcessInputSystem (в начале FixedUpdate)

Resource: InputSourceHandle (Box<dyn InputSource>)

Алгоритм:
  1. commands = input_source.poll_commands()
  2. Для каждой команды:
     - Конвертировать в Bevy Event (MoveCommand, FireCommand, etc.)
     - Отправить в Event<T> queue
  3. Другие Bevy системы обрабатывают события
```

**GodotInputSource реализация:**
```
(Текстовое описание)

struct GodotInputSource {
    godot_input: Gd<Input>,
    command_buffer: Vec<InputCommand>,
}

Реализация poll_commands:
  1. Читать Godot Input.is_action_pressed("move_forward")
  2. Конвертировать в InputCommand::MoveForward
  3. Вернуть Vec<InputCommand>

Godot InputMap:
  - Определяет кнопки в project.godot
  - GodotInputSource просто читает actions (не знает про конкретные кнопки)
```

---

## 8. Производительность и оптимизации

### Batch updates (критично для 1000+ entities)

**Проблема:** Отправлять Transform по одному = overhead (function calls, lock contention)

**Решение: Batching**
```
(Текстовое описание)

Bevy система:
  1. Собрать все Changed<Transform> в Vec<TransformUpdate>
  2. Один вызов: client.update_transform_batch(&updates)

GodotClient обрабатывает batch:
  - Один lock на entity_nodes HashMap
  - Пройти по всем updates в tight loop
  - Unlock после обработки batch

Выигрыш: 10-50x меньше overhead при 1000 entities vs индивидуальные вызовы
```

### Prefab caching

**Проблема:** Загружать prefab при каждом spawn = медленно

**Решение:**
```
(Текстовое описание)

GodotClient.prefab_cache: HashMap<String, Gd<PackedScene>>

При spawn_visual:
  1. Проверить cache
  2. Если нет → load + insert в cache
  3. Instantiate из cached сцены

Invalidation:
  - Hot-reload: очистить cache при изменении .tscn файла
  - Memory limit: LRU eviction если cache > N prefab'ов
```

### Async loading (для больших ассетов)

**Проблема:** Загрузка большого prefab блокирует frame (lag spike)

**Решение: Async trait methods (Rust 2024)**
```
(Текстовое описание)

trait PresentationClient {
    async fn spawn_visual_async(
        &mut self,
        entity_id: EntityId,
        prefab_path: String,
        initial_transform: Transform3D
    ) -> Result<()>;
}

Использование:
  - Bevy система отправляет spawn command
  - Клиент загружает prefab асинхронно (background thread)
  - Когда готов → instantiate + notify симуляцию через событие
  - До загрузки: показать placeholder (простой куб или loading icon)
```

---

## 9. Сериализация и network protocol

### Для multiplayer: protocol types должны быть serializable

**Требования:**
- TransformUpdate, SpawnVisualCommand, etc. → serialize в binary
- Отправка по сети: Server → Clients
- Клиенты применяют команды через PresentationClient

**Формат:**
```
(Текстовое описание)

Derive serde::Serialize + serde::Deserialize для всех protocol types

Использовать bincode для binary serialization (эффективнее JSON)

Network packet:
  - Header: packet_type (TransformBatch, SpawnVisual, VFX)
  - Payload: serialized data
  - CRC32 checksum для валидации

Server:
  - Собрать все PresentationClient команды за tick
  - Serialize в packet batch
  - Отправить всем подключённым клиентам

Client:
  - Deserialize packets
  - Применить команды через local PresentationClient implementation
```

---

## 10. Миграция: как заменить Godot на другой движок

### Сценарий: Godot → Bevy Renderer

**Шаг 1: Создать voidrun_bevy_client crate (1 неделя)**
- Имплементировать PresentationClient trait
- spawn_visual: загрузка .gltf через Bevy AssetServer
- update_transform_batch: обновление Transform в Render World
- trigger_vfx: Bevy particles/post-processing

**Шаг 2: Портировать visual prefab'ы (1-2 недели)**
- Godot .tscn → экспорт .gltf из Blender
- Анимации уже в .gltf (universal format)
- Материалы: воссоздать в Bevy (StandardMaterial или custom shaders)

**Шаг 3: Портировать UI (1 неделя)**
- Godot Control nodes → Bevy UI (ButtonBundle, TextBundle)
- Layout можно воссоздать (anchors/margins → Bevy flex layout)
- Dynamic UI generation: тот же protocol (update_ui_list работает аналогично)

**Шаг 4: Input (3-5 дней)**
- GodotInputSource → BevyInputSource
- Читать Bevy Input resource вместо Godot Input singleton
- Маппинг клавиш: Bevy InputMap plugin (аналог Godot InputMap)

**Шаг 5: Тестирование (1 неделя)**
- Запустить игру с BevyClient
- Визуально сравнить с Godot версией
- Performance profiling (может быть быстрее/медленнее)
- Фиксить edge cases

**Итого: 4-6 недель** для полной миграции

**Что НЕ меняется:**
- ✅ voidrun_simulation crate (0 изменений)
- ✅ voidrun_protocol trait (0 изменений)
- ✅ Логика игры, AI, физика, экономика (всё в Bevy, не трогается)

---

### Сценарий: Godot → Unreal/Unity

**Сложность выше** (нужен FFI слой), но принцип тот же:

**Шаг 1: C FFI bridge (2-3 недели)**
- Rust → C API export (через `extern "C"` functions)
- Wrap PresentationClient trait в C функции
- Unreal/Unity вызывает C API через Plugin

**Шаг 2: Реализация на стороне движка (3-4 недели)**
- Unreal: Blueprint nodes вызывают C API
- Unity: C# P/Invoke для вызова Rust DLL
- Spawn визуальных объектов в нативном формате движка

**Шаг 3: Портирование контента (4-6 недель)**
- Prefab'ы: .gltf импорт в Unreal/Unity
- UI: перерисовка в нативных инструментах
- Materials: воссоздание

**Итого: 3-4 месяца** (но возможно, если архитектура правильная)

---

## 11. Риски и trade-offs

### Риск 1: Over-abstraction (слишком общий trait)

**Симптом:** PresentationClient имеет 50+ методов, покрывает все возможные случаи

**Проблема:**
- Сложно имплементировать (каждый новый движок = писать 50 функций)
- Performance overhead (indirect calls, dynamic dispatch)

**Митигация:**
- Держать trait минимальным (~10-15 core методов)
- Extension points через generic параметры (custom_params: HashMap)
- Специфичные фичи движка — опциональные trait extensions

---

### Риск 2: Protocol evolution (breaking changes)

**Симптом:** Добавили новое поле в TransformUpdate → старые клиенты не работают

**Проблема:**
- Network protocol версии
- Save file compatibility

**Митигация:**
- Версионирование: protocol_version field в каждом пакете
- Backward compatibility: опциональные поля через Option<T>
- Migrations: конвертеры старых версий в новые

---

### Риск 3: Performance overhead trait calls

**Симптом:** Много мелких вызовов client.update_transform() → overhead

**Проблема:**
- Dynamic dispatch (`dyn Trait`) имеет небольшой cost
- Lock contention если PresentationClient shared между threads

**Митигация:**
- Batching: всегда использовать _batch варианты методов
- Static dispatch где возможно (generic <C: PresentationClient> вместо Box<dyn>)
- Benchmark: сравнить с baseline (direct Godot calls vs через trait)

---

## 12. Тестирование абстракции

### Unit тесты (MockClient)

**Что проверять:**
```
(Текстовое описание)

Тест: spawn entity → проверить MockClient.spawned_entities.contains(id)
Тест: update transform → проверить последний transform_log entry
Тест: trigger VFX → проверить vfx_log содержит правильный effect_type
Тест: batch updates → проверить все entities обновились
```

### Integration тесты (реальный клиент)

**GodotClient integration test:**
```
(Текстовое описание)

1. Запустить Godot в headless mode (без окна, но с scene tree)
2. Создать GodotClient
3. Отправить spawn_visual команду
4. Проверить: godot_root.get_child_count() увеличился
5. Проверить: entity_nodes mapping заполнен
6. Отправить update_transform
7. Проверить: node.global_position изменился
```

### Property тесты (детерминизм)

**Проверка:** Два клиента (Godot vs Mock) получают одинаковые команды → должны быть в консистентном состоянии

```
(Текстовое описание)

1. Запустить симуляцию с GodotClient, записать все protocol commands
2. Replay записанных команд в MockClient
3. Сравнить: количество spawned entities, финальные transforms (должны совпадать)
```

---

## 13. Документация protocol

### API Reference (auto-generated)

**Использовать cargo doc:**
- Документировать каждый метод PresentationClient trait
- Примеры использования в docstrings
- Ссылки на data structures

**Формат:**
```
(Текстовое описание)

/// Spawns a visual representation of an entity.
///
/// # Arguments
/// * `entity_id` - Stable ID from simulation
/// * `prefab_path` - Universal path to visual prefab (e.g. "prefabs/ship_engine")
/// * `initial_transform` - Initial position/rotation
///
/// # Example
/// Симуляция создала корабль, нужно показать его в игре:
/// client.spawn_visual(12345, "prefabs/player_ship", transform)?;
///
/// # Errors
/// Returns Err if prefab_path not found or spawn failed
fn spawn_visual(
    &mut self,
    entity_id: EntityId,
    prefab_path: String,
    initial_transform: Transform3D
) -> Result<()>;
```

### Migration Guide (для портирования)

**Документ: "How to Implement PresentationClient"**

**Шаги:**
1. Создать новый crate `voidrun_your_engine_client`
2. Добавить зависимость на `voidrun_protocol`
3. Имплементировать trait для каждого метода (checklist)
4. Тестировать через MockClient comparison
5. Интеграция в main binary

**Чеклист методов (must implement):**
- [ ] spawn_visual
- [ ] despawn_visual
- [ ] update_transform_batch
- [ ] play_animation
- [ ] trigger_vfx
- [ ] play_sound_3d
- [ ] update_ui_text
- [ ] update_ui_list

**Опциональные:**
- [ ] set_camera_target (если движок поддерживает camera control)
- [ ] shake_camera

---

## 14. Альтернативные подходы (рассмотренные и отвергнутые)

### Подход A: Прямые вызовы Godot API из Bevy систем

**Описание:** Bevy системы напрямую вызывают gdext функции

**Почему отвергнут:**
- ❌ Жёсткая привязка к Godot (невозможно заменить)
- ❌ Headless тесты требуют мокирования всего Godot API
- ❌ Нет явного контракта (protocol разбросан по всему коду)

---

### Подход B: Message Queue (async channel)

**Описание:** Bevy отправляет команды в channel, Godot читает асинхронно

**Trade-offs:**
- ✅ Полностью async (не блокирует Bevy thread)
- ⚠️ Сложнее debugging (команды теряются в queue)
- ⚠️ Latency (команды не мгновенно применяются)

**Почему не выбран:**
- Для VOIDRUN sync вызовы достаточны (Bevy и Godot в одном процессе)
- Можно добавить async позже (trait может иметь async методы)

---

### Подход C: Shared Memory (no-copy sync)

**Описание:** Bevy пишет Transform в shared memory, Godot читает напрямую

**Trade-offs:**
- ✅ Zero-copy (максимальная производительность)
- ⚠️ Сложная синхронизация (locks, atomic operations)
- ⚠️ Платформо-зависимо (shared memory на Windows vs Linux)

**Почему не выбран:**
- Overkill для single-machine клиента
- Может быть использован для network server (отправка snapshots клиентам)

---

## 15. Roadmap и следующие шаги

### MVP (Фаза 1): Минимальный trait (2-3 недели)

**Задачи:**
1. Создать `voidrun_protocol` crate
2. Определить PresentationClient trait (5-7 core методов)
3. Реализовать MockClient (для тестов)
4. Интегрировать в voidrun_simulation (заменить прямые вызовы)
5. Unit тесты: MockClient работает корректно

**Критерий готовности:** Headless тесты проходят с MockClient

---

### Фаза 2: GodotClient implementation (1-2 недели)

**Задачи:**
1. Создать `voidrun_godot_client` crate
2. Имплементировать все методы PresentationClient
3. Prefab loading + caching
4. Transform batching оптимизация
5. Integration тесты: Godot визуализирует корректно

**Критерий готовности:** Игра запускается с Godot, визуал синхронизирован

---

### Фаза 3: Protocol extension (по мере добавления фич)

**Задачи:**
- Добавить методы для новых фич (camera shake, UI notifications)
- Версионирование protocol
- Network serialization (для multiplayer)

**Ongoing:** Protocol эволюционирует вместе с игрой

---

### Фаза 4 (опционально): BevyClient implementation (3-4 недели)

**Если решим мигрировать на Bevy Renderer:**
1. Создать `voidrun_bevy_client` crate
2. Имплементировать PresentationClient
3. Портировать prefab'ы + UI
4. Performance comparison с Godot
5. Выбрать финальный движок

---

## 16. Заключение и ключевые takeaways

### Что даёт абстракция:

✅ **Портируемость:** Замена движка = 4-6 недель вместо полной переписывания
✅ **Тестируемость:** MockClient для headless CI без визуального движка
✅ **Ясность:** Явный protocol документирует контракт симуляция ↔ презентация
✅ **Изоляция:** Bugs в визуале не ломают симуляцию (разные crates)

### Стоимость:

⚠️ **Boilerplate:** Отдельный crate + trait definitions (~1000 LOC overhead)
⚠️ **Maintenance:** При добавлении фич обновлять trait + все реализации
⚠️ **Indirection:** Небольшой performance overhead (negligible при batching)

### Рекомендация:

**Для VOIDRUN: делать абстракцию** — проект долгосрочный, требования могут измениться (например, захочется AAA графику через Unreal). Стоимость абстракции окупается гибкостью.

**Для прототипов/gamejam:** можно пропустить — прямые вызовы движка быстрее итерации.

---

## 17. Ссылки и источники

### Rust Trait Patterns (2024)

**Rust Blog:**
- Abstraction without overhead (traits): https://blog.rust-lang.org/2015/05/11/traits.html
- Rust 2024 edition (impl Trait improvements): https://blog.rust-lang.org/2024/08/12/Project-goals/

**Rust Design Patterns:**
- https://rust-unofficial.github.io/patterns/

### Game Engine Abstractions

**bgfx (C++ reference):**
- GitHub: https://github.com/bkaradzic/bgfx
- Philosophy: "Bring Your Own Engine" style rendering library

**Bevy Renderer Architecture:**
- Bevy 0.6 (wgpu direct use): https://bevy.org/news/bevy-0-6/
- Render Architecture Overview: https://bevy-cheatbook.github.io/gpu/intro.html

**Layered Game Engine Architecture:**
- GAMES104 02: https://alalba221.github.io/blog/engine/LayeredArchitectureOfGameEngine
- Isetta Engine Architecture: https://isetta.io/blogs/engine-architecture/

---

**Следующий шаг:** Начать Фазу 1 (создание voidrun_protocol crate) параллельно с другими архитектурными блоками (Content Pipeline, AI systems).
