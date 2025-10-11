# VOIDRUN: Godot-Rust Integration Architecture (Rust-Only)

## Дата создания: 2025-10-07
## Последнее обновление: 2025-01-10 (аудит + Domain Events + NO GDScript)
## Версия: 2.2
## Статус: Validated + Bevy Events Architecture + Rust-Only

---

## 1. Цель: 100% Rust, 0% GDScript

### Рекомендация

**Архитектура "Rust-Centric with Godot as I/O"** — вся игровая логика в Bevy ECS (Rust), Godot используется только как тонкий презентационный слой (рендер, input, audio). **GDScript не используется ВООБЩЕ** — даже сцены создаются процедурно из Rust через godot-rust (gdext), все Godot nodes пишутся на Rust.

### Обоснование из индустрии (2024-2025)

**godot-rust (gdext) capabilities:**
- ✅ **Hot-reloading** (2024): не нужно перезапускать редактор при изменениях Rust
- ✅ **Type-safe signals** (May 2025): signals с проверкой типов (в отличие от GDScript)
- ✅ **Procedural scene building**: полный доступ к созданию Node'ов из Rust (`Node3D::new_alloc()`, `add_child()`)
- ✅ **OnReady/OnEditor fields**: автоинициализация через `#[init(node = "path")]`
- ✅ **Async/await support**: нативная поддержка асинхронности через signals
- ✅ **Minimal boilerplate**: `#[class(init)]` для авто-инициализации
- ✅ **Composition over inheritance**: `Base<T>` вместо GDScript наследования

**godot-bevy библиотека (v0.8+, 2024):**
- ✅ Bevy ECS интегрирован как Godot Node (autoload)
- ✅ Godot nodes ↔ Bevy entities mapping
- ✅ Node properties ↔ Bevy Components sync
- ✅ Godot Signals → Bevy Events
- ✅ Plugin system: opt-in фичи (Transform sync, Audio, Input)
- ✅ Работает с Bevy 0.16 + Godot 4

**Headless server support:**
- ✅ Godot 4 `--headless` mode: отключает рендер, но GDExtension работает
- ✅ Идеально для dedicated servers + CI тестов

### Trade-offs

**За Rust-only подход:**
- ✅ Детерминизм: вся логика в одном языке (Rust)
- ✅ Типобезопасность: компилятор ловит ошибки до runtime (в отличие от GDScript)
- ✅ Performance: прямой доступ к Bevy ECS без лишних слоёв
- ✅ Headless testing: Bevy симуляция работает без Godot
- ✅ Single source of truth: state в Bevy, Godot только визуализирует
- ✅ Модульность: godot-rust GodotClass = композиция (не inheritance как в GDScript)

**Против (сложности):**
- ⚠️ Compile time: Rust медленнее компилируется (но hot-reload помогает)
- ⚠️ Godot editor workflow: сцены создаются в Rust, не видны в редакторе
- ⚠️ Onboarding: дизайнеры/художники не могут трогать логику в редакторе
- ⚠️ Debugging: ошибки в Rust требуют перекомпиляции

---

## 1.5 Command/Event Architecture (ADR-004, 2025-01-10)

### Решение: Bevy Events вместо trait-based abstraction

**Прежняя концепция (отклонена):**
- GodotBridge trait (как PresentationClient) — оверинжиниринг (ADR-002 POSTPONED)
- Custom Command/Event queues с handlers — громоздко

**Текущая архитектура (ПРИНЯТА):**
- **Прямая зависимость** voidrun_godot → voidrun_simulation (tight coupling — это OK)
- **Bevy Events** для всех Command/Event потоков
- **Change Detection** (`Changed<T>` queries) для sync

### Event Types

**ОБНОВЛЕНО (2025-01-10):** Domain Events вместо одного GodotInputEvent.

**Domain Events из Godot в ECS** (разделение по доменам):
```rust
// events/combat.rs
#[derive(Event, Clone, Debug)]
pub enum GodotCombatEvent {
    WeaponHit { attacker: Entity, victim: Entity, hitbox_name: String },
    Parry { defender: Entity, attacker: Entity },
}

// events/animation.rs
#[derive(Event, Clone, Debug)]
pub enum GodotAnimationEvent {
    AnimationFinished { entity: Entity, animation: String },
    AnimationTrigger { entity: Entity, trigger_name: String },
}

// events/transform.rs
#[derive(Event, Clone, Debug)]
pub enum GodotTransformEvent {
    ZoneTransition { entity: Entity, new_chunk: ChunkCoord },
    PostSpawn { entity: Entity, actual_position: Vec3 },
    ArrivedAtDestination { entity: Entity },
}

// events/ai.rs
#[derive(Event, Clone, Debug)]
pub enum GodotAIEvent {
    ActorSpotted { observer: Entity, target: Entity },
    ActorLost { observer: Entity, target: Entity },
}

// events/input.rs
#[derive(Event, Clone, Debug)]
pub struct PlayerInputEvent {
    movement: Vec3,
    look_dir: Vec3,
    jump: bool,
    dodge: bool,
}
```

**Domain Events внутри ECS** (модульные события между системами):
```rust
// combat/events.rs
#[derive(Event)]
pub struct DamageDealt { attacker: Entity, victim: Entity, amount: f32 }

// ai/events.rs
#[derive(Event)]
pub struct ZoneTransitionEvent { entity: Entity, from: ChunkCoord, to: ChunkCoord }
```

### Sync через Change Detection

**Godot sync системы** используют Bevy встроенную Change Detection:

```rust
// voidrun_godot/src/animation_sync.rs
pub fn sync_ai_animations(
    // Changed<AIState> — только entity где AIState изменился
    query: Query<(Entity, &AIState), Changed<AIState>>,
    visuals: Res<VisualRegistry>,
) {
    for (entity, state) in query.iter() {
        if let Some(node) = visuals.get(entity) {
            let anim = node.get_node_as::<AnimationPlayer>("AnimationPlayer");
            match state {
                AIState::Attacking => anim.play("attack_swing".into()),
                AIState::Idle => anim.play("idle".into()),
                // ...
            }
        }
    }
}
```

**Как работает:**
- Bevy отслеживает изменения компонентов автоматически (tick counter)
- `Changed<T>` фильтрует только entity с `component.tick > system.last_run_tick`
- Sync системы обрабатывают только изменённые компоненты (не каждый frame для всех)

**Преимущества:**
- ✅ Zero boilerplate (встроенный механизм Bevy)
- ✅ Модульность через типы (не через traits)
- ✅ Testability (mock events без Godot)
- ✅ KISS principle (нет промежуточных абстракций)

**См. также:** [ADR-004: Command/Event Architecture](../decisions/ADR-004-command-event-architecture.md)

---

## 2. Три архитектурных паттерна godot-rust (выбор для VOIDRUN)

### Паттерн 1: Rust as GDScript Extension (❌ НЕ для нас)

**Описание:**
- Основная логика в GDScript
- Rust только для performance-critical участков (физика, AI, pathfinding)

**Почему НЕ подходит:**
- Нарушает детерминизм (GDScript + Rust = два источника truth)
- Не минимизирует GDScript (наоборот, GDScript основной)
- Headless тесты невозможны (логика завязана на Godot сцены)

---

### Паттерн 2: Rust Scripts for Scene Nodes (❌ НЕ для нас)

**Описание:**
- Каждый Node имеет Rust скрипт (аналог GDScript)
- Godot сцены создаются в редакторе, логика в Rust

**Почему НЕ подходит:**
- Всё ещё привязка к Godot scene tree (не изолированная симуляция)
- Сложно синхронизировать с Bevy ECS (два разных Entity подхода)
- Не позволяет headless симуляцию (нужна вся Godot scene инфраструктура)

---

### Паттерн 3: Rust-Centric with Godot as I/O (✅ ВЫБРАН)

**Описание:**
- Вся игровая логика в Rust (Bevy ECS)
- Godot только как "презентационный клиент":
  - Input collection → отправка в Bevy
  - Rendering: получение Transform updates от Bevy
  - Audio/VFX: реакция на Bevy Events
- Нет GDScript вообще
- Сцены создаются процедурно из Rust

**Почему подходит:**
- ✅ Изолированная Bevy симуляция (headless тесты без Godot)
- ✅ Детерминизм (весь state в Bevy)
- ✅ Bevy ECS native workflow (не подстраиваемся под Godot)
- ✅ Rollback netcode возможен (physics-architecture.md)
- ✅ Single codebase для клиента и dedicated server

**Архитектурная диаграмма (текстом):**

```
[Player Input (Godot)]
    ↓
[GodotInputBridge (Rust GDExtension)]
    ↓ Commands/Events
[Bevy ECS Simulation (Authoritative)]
    ↓ State Changes (Position, Health, etc.)
[GodotRenderBridge (Rust GDExtension)]
    ↓
[Godot Scene Tree (Visual Output)]
```

---

## 3. godot-bevy: готовое решение или custom bridge?

### Вариант A: Использовать godot-bevy библиотеку

**Что даёт godot-bevy (v0.8+):**
- Bevy App как Godot Node (добавляется через autoload)
- Автосинхронизация Transform: Bevy GlobalTransform → Godot Node3D
- Input bridge: Godot Input → Bevy InputEvents
- Audio bridge: Bevy audio events → Godot AudioStreamPlayer
- Модульная система: opt-in plugins (GodotTransformsPlugin, GodotAudioPlugin)

**Trade-offs:**
- ✅ Быстрый старт: работает из коробки
- ✅ Поддерживается комьюнити (совместимость с новыми версиями Bevy/Godot)
- ⚠️ Зависимость от внешней библиотеки (breaking changes возможны)
- ⚠️ Может не подходить для специфичных нужд (rollback netcode, custom sync logic)

---

### Вариант B: Custom bridge через gdext (❌ сложнее, но гибче)

**Что писать самим:**
- Rust GDExtension Node: `VoidrunSimulationNode`
- Внутри: Bevy App с MinimalPlugins (без рендера)
- Ручная синхронизация: Bevy Query → Godot Node properties
- Custom protocol: события Bevy → Godot signals

**Trade-offs:**
- ✅ Полный контроль над sync logic (оптимизация bandwidth)
- ✅ Custom serialization (binary vs JSON)
- ✅ Легче интегрировать rollback (свой protocol)
- ⚠️ Больше boilerplate кода
- ⚠️ Нужно поддерживать при обновлениях Bevy/Godot

---

### Рекомендация для VOIDRUN

**MVP (первые 2-3 месяца): godot-bevy библиотека**
- Быстрый старт, можно сразу видеть результат
- Проверка концепции: работает ли Rust-centric подход
- Если обнаружим ограничения → мигрируем на custom bridge

**Production (после MVP): Custom bridge — ОБЯЗАТЕЛЬНО**
- **Критично для rollback netcode:** godot-bevy автосинхронизация конфликтует с rollback
- Когда нужна оптимизация bandwidth (отправлять только Changed компоненты)
- Когда godot-bevy не покрывает специфичные нужды (lag compensation, etc.)

**⚠️ КРИТИЧЕСКИЙ РИСК: godot-bevy + Rollback Netcode (аудит 2025)**

**Проблема:**
- godot-bevy `GodotTransformsPlugin` автоматически синхронизирует Bevy Transform → Godot Node3D
- При rollback (откат на N тиков назад) Bevy откатывает позиции в прошлое
- godot-bevy может отправить Transform updates **из будущего** (после rollback, до re-simulation)
- **Результат:** визуальные глитчи (entity телепортируется, анимации дёргаются)

**Обнаружение:**
- Playtesting: если видим "резкие прыжки" entity при сетевых задержках
- Метрика: частота rollback artifacts >5/sec → неприемлемо

**Решение для MVP:**
- Использовать godot-bevy, но **отключить** `GodotTransformsPlugin`
- Реализовать custom sync: отправлять только **confirmed state** (после re-simulation)

**Решение для Production:**
- Полный custom bridge с контролем:
  - `send_transforms_only_after_rollback_complete()`
  - `interpolate_visual_positions()` на клиенте для сглаживания
  - Отправлять только Changed<Transform> (bandwidth оптимизация)

---

## 4. Процедурное создание сцен из Rust (без Godot редактора)

### Проблема

Godot редактор удобен для ручного placement объектов, но:
- Сцены в `.tscn` файлах = контент, не логика
- Для data-driven игры (items/NPC из YAML) нужно создавать сцены программно
- Художники могут создавать visual prefab'ы, но логика spawn'а в Rust

### Решение: Hybrid Workflow

**Godot редактор для:**
- Visual prefab'ы (3D модели + материалы + анимации)
- UI layouts (основная структура меню, HUD)
- Lighting/environment setups

**Rust для:**
- Spawning entities (какие prefab'ы создать, где, когда)
- Runtime composition (добавление компонентов к Node'ам)
- Dynamic UI (генерация inventory списков, market prices)

### Пример workflow (текстовое описание)

**Шаг 1: Художник создаёт prefab в Godot**
- Файл: `res://prefabs/ship_engine.tscn`
- Содержит: MeshInstance3D + CollisionShape3D + AnimationPlayer
- НЕ содержит логику (нет скриптов)

**Шаг 2: Rust спавнит entity из prefab'а**
- Загрузка: `load("res://prefabs/ship_engine.tscn")`
- Instantiate: `scene.instantiate_as::<Node3D>()`
- Добавление в сцену: `parent.add_child(instance)`
- Привязка к Bevy: создать Bevy Entity с компонентом `GodotNodeRef { node_path }`

**Шаг 3: Bevy контролирует, Godot визуализирует**
- Bevy система обновляет `Transform` компонент
- GodotRenderBridge читает Changed<Transform> → обновляет Node3D.transform
- Bevy события (ShipEngineDestroyed) → Godot играет анимацию explosion

---

## 5. GDExtension API: контракты между Rust и Godot

### Ключевые GDExtension типы (godot-rust gdext)

**Gd<T>:** smart pointer на Godot объект (аналог Rc/Arc)
- Пример: `Gd<Node3D>` = reference на Node3D в Godot scene tree

**Base<T>:** доступ к base class методам (composition вместо наследования)
- Пример: в `#[class]` struct имеет поле `base: Base<Node3D>` → доступ к position, rotation, etc.

**OnReady<T>:** автоинициализация при _ready()
- Пример: `#[init(node = "Sprite")] sprite: OnReady<Gd<Sprite2D>>`

**Signals:** type-safe pub/sub
- Rust emit: `self.base_mut().emit_signal("health_changed", &[damage.to_variant()])`
- Rust subscribe: через `#[signal]` attribute → автосоздание signal definition

### GDScript Usage Policy

**СТРОГО ЗАПРЕЩЕНО:**
- ❌ Писать любой GDScript код
- ❌ Использовать .gd файлы для логики
- ❌ Создавать GDScript скрипты в редакторе

**ВСЁ пишется на Rust через godot-rust:**
- ✅ Godot nodes → `#[derive(GodotClass)]` в Rust
- ✅ Signals → Rust callable methods
- ✅ Scene building → процедурно из Rust
- ✅ Autoload → Rust singleton через GDExtension
- ✅ **Единственный конфиг:** `project.godot` (autoload для Rust Node)

**Дополнительно:**
- ✅ **Scene files (*.tscn):** только visual prefab'ы, нет скриптов
- ✅ **Input mapping:** `project.godot` InputMap (но можно и из Rust переопределять)

---

## 6. Hot-Reload workflow (итерация без перезапуска)

### Возможности godot-rust hot-reload (2024)

**Что работает:**
- Изменения в Rust коде → перекомпиляция
- Godot автоматически перезагружает GDExtension library
- State сохраняется (если правильно реализован serialization)

**Что НЕ работает:**
- Изменение сигнатуры `#[class]` struct (добавление/удаление полей) → требует restart
- Изменение Godot сцен (*.tscn) → нужен reload scene (но не restart editor)

### Workflow для быстрых итераций

**Шаг 1: Rust код изменён (добавили систему в Bevy)**
- `cargo build` в фоне (watch mode)
- Godot перезагружает .so/.dll

**Шаг 2: Тестирование в игре**
- Нажать F5 в Godot → сцена перезапускается с новым кодом
- Если нужно сохранить state → использовать Bevy snapshot system

**Шаг 3: Visual prefab изменён (художник обновил 3D модель)**
- Godot автоимпорт .gltf/.fbx
- Rust код не трогается
- F5 → новая модель в игре

**Итого:** hot-reload для Rust логики работает, visual итерации тоже быстрые

---

## 7. Headless server & CI testing

### Godot Headless Mode для Servers

**Команда запуска:**
```
(текстовое описание: godot --headless флаг)
Отключает рендер, окно, GPU
GDExtension продолжает работать
Bevy симуляция запускается как обычно
```

**Use cases:**
- Dedicated multiplayer server (нет клиентов на той же машине)
- CI тесты (GitHub Actions без GPU)
- Batch processing (генерация процедурного контента)

### CI Pipeline Architecture

**Тест 1: Pure Bevy (без Godot)**
- Cargo test на Rust codebase
- Bevy симуляция с MinimalPlugins
- Property тесты детерминизма
- **Скорость:** быстро (нет Godot overhead)

**Тест 2: Godot Headless Integration**
- Запуск Godot --headless с GDExtension
- Проверка что Rust ↔ Godot bridge работает
- События корректно передаются
- **Скорость:** медленнее, но покрывает интеграцию

**Тест 3: Visual Regression (опционально)**
- Godot с рендером (нужен GPU runner в CI)
- Snapshot тесты UI/визуала
- **Скорость:** очень медленно, делать редко

---

## 8. UI Generation from Rust (без UI editor'а в Godot)

### Проблема

Godot UI редактор удобен, но:
- Для data-driven UI (inventory items, market listings) нужна генерация
- Static layouts в .tscn, dynamic content из Rust

### Решение: Template-based UI

**Godot создаёт template:**
- Файл: `res://ui/inventory_slot.tscn`
- Содержит: TextureRect (icon) + Label (name) + Button (use)
- NO logic, только structure

**Rust генерирует instances:**
- Загрузка template: `load("res://ui/inventory_slot.tscn")`
- Для каждого item в inventory:
  - Instantiate slot
  - Заполнить icon, name из item data (YAML)
  - Добавить в inventory container
  - Подключить signal "pressed" → Bevy event UseItem

**Альтернатива: Полная генерация из Rust**
- Создать Control nodes через `Control::new_alloc()`
- Выставить anchors, margins, text
- Trade-off: больше кода, но полная гибкость

---

## 9. Риски и митигация

### Риск 1: godot-bevy breaking changes

**Симптом:** Обновление Bevy/Godot ломает godot-bevy
**Вероятность:** Средняя (библиотека молодая)
**Статус (аудит 2025):** ✅ Godot 4.3→4.4 безопасна, GDExtension API стабилен с 4.1
**Митигация:**
- Закрепить версии в Cargo.toml: `godot-bevy = "=0.8"`, `godot = "=4.3"`, `bevy = "=0.16"`
- Если godot-bevy тормозит — мигрировать на custom bridge
- Тестировать обновления в отдельной ветке перед merge

---

### Риск 3: godot-bevy Rollback Конфликт (КРИТИЧНЫЙ, аудит 2025)

**Симптом:** При rollback netcode entity визуально "телепортируются" или дёргаются
**Вероятность:** **ВЫСОКАЯ** — автосинхронизация не знает про rollback
**Обнаружение:**
- Playtesting с симуляцией лага (50-150ms)
- Визуально видно: entity "прыгает назад" при rollback, затем резко вперёд
- Метрика: rollback artifacts >5/sec

**Митигация:**
- **Краткосрочная (MVP):** отключить `GodotTransformsPlugin`, написать custom sync (100-200 строк)
- **Долгосрочная (Production):** полный custom bridge с rollback-aware логикой
- **Критерий миграции:** если видим artifacts в первом playtest → немедленно custom bridge

**Cost миграции:** 1-2 недели (после Фазы 4 godot-bevy integration)

---

### Риск 2: Slow compile times

**Симптом:** `cargo build` занимает 30+ секунд, тормозит итерации
**Вероятность:** Высокая (Rust + большая кодовая база)
**Митигация:**
- Использовать `cargo watch` для инкрементальных сборок
- Split кодовая база на crates (voidrun_physics, voidrun_ai — отдельно)
- Использовать `mold` linker (Linux) или `lld` (Windows) для ускорения линковки
- Conditional compilation: feature flags для отключения ненужных систем в dev сборках

---

### Риск 3: Onboarding non-Rust contributors

**Симптом:** Дизайнеры/художники не могут добавить простую логику
**Вероятность:** Высокая (Rust имеет learning curve)
**Митигация:**
- Визуальный контент остаётся в Godot (prefab'ы, анимации)
- Data-driven подход: добавление item'ов через YAML (не нужен Rust)
- Документация: примеры "как добавить новое оружие" (шаг за шагом)
- Future: visual scripting через Rust macros (если станет критично)

---

### Риск 4: Debugging сложнее чем GDScript

**Симптом:** Godot debugger не показывает Rust stack traces
**Вероятность:** Средняя
**Митигация:**
- Использовать rust-gdb / lldb для Rust-side debugging
- Log-based debugging: `tracing` crate с фильтрами
- Godot remote debugger для scene tree inspection (визуальная часть)
- Asserts и invariants в Rust коде (fail fast)

---

## 10. План внедрения (поэтапный)

### Фаза 0: Proof of Concept (1 неделя)

**Задачи:**
1. Создать новый Godot 4 проект
2. Добавить godot-rust (gdext) через Cargo
3. Создать простой Rust GDExtension Node (`HelloWorld`)
4. Зарегистрировать в Godot, добавить в сцену
5. Проверить hot-reload: изменить Rust → rebuild → увидеть изменения в Godot

**Критерий готовности:** Godot показывает Node созданный из Rust, hot-reload работает

---

### Фаза 1: Bevy Integration (1-2 недели)

**Вариант A (godot-bevy):**
1. Добавить `godot-bevy` в Cargo.toml
2. Создать Bevy App с basic systems (spawn cube entity)
3. Использовать `GodotTransformsPlugin` для автосинхронизации
4. Увидеть cube в Godot, двигающийся из Bevy логики

**Вариант B (custom bridge):**
1. Создать `VoidrunSimulationNode` (GDExtension)
2. Внутри: Bevy App (MinimalPlugins)
3. Система `SyncTransformsToGodot`: Query<(Entity, &Transform)> → Godot Node3D updates
4. Manual тестирование: spawn Bevy entity → появляется Godot Node

**Критерий готовности:** Bevy entity движется, Godot Node синхронизирован визуально

---

### Фаза 2: Input Bridge (1 неделя)

**Задачи:**
1. Godot получает input (keyboard/mouse) через InputMap
2. GDExtension функция `process_input()` читает Godot Input
3. Конвертирует в Bevy Events: `PlayerMoveCommand`, `PlayerFireCommand`
4. Bevy система обрабатывает → entity двигается

**Критерий готовности:** Игрок контролирует entity через keyboard, реакция <50ms

---

### Фаза 3: Visual Prefab Spawning (1 неделя)

**Задачи:**
1. Художник создаёт `res://prefabs/player_ship.tscn` (3D модель + анимации)
2. Rust система `SpawnPlayerSystem`:
   - Создаёт Bevy Entity с компонентами (Transform, Velocity, Health)
   - Загружает prefab: `load("res://prefabs/player_ship.tscn")`
   - Instantiate + add_child в Godot
   - Сохраняет mapping: Bevy Entity ↔ Godot Node path
3. Проверить что Transform sync работает (Bevy двигает, Godot визуализирует)

**Критерий готовности:** Корабль появляется из YAML данных, управляется через Bevy

---

### Фаза 4: Event-Driven VFX (1 неделя)

**Задачи:**
1. Bevy событие: `ProjectileHit { entity, position, damage }`
2. GodotRenderBridge подписан на события
3. При получении события:
   - Найти Godot Node по entity
   - Trigger AnimationPlayer ("hit_flash")
   - Spawn particles (explosion prefab)
4. VFX живёт только на клиенте (не влияет на Bevy simulation)

**Критерий готовности:** Попадание снаряда → искры/анимация в Godot, симуляция не тормозит

---

### Фаза 5: Dynamic UI Generation (1-2 недели)

**Задачи:**
1. Создать UI template: `res://ui/item_slot.tscn`
2. Rust система `GenerateInventoryUI`:
   - Читает Bevy resource `PlayerInventory`
   - Для каждого item:
     - Instantiate slot template
     - Заполнить icon (загрузка из `res://icons/{item_id}.png`)
     - Заполнить name/description из item data
     - Подключить signal `pressed` → Bevy event `UseItem`
3. При изменении inventory (Added/Removed items) → перегенерировать UI

**Критерий готовности:** Inventory UI обновляется автоматически при изменении данных в Bevy

---

### Фаза 6: Headless Server (1 неделя)

**Задачи:**
1. Создать отдельный binary target: `voidrun_server`
2. Запускать Godot с `--headless`
3. Bevy App запускается как обычно (вся логика работает)
4. Отключить GodotRenderBridge (нет визуала)
5. Включить NetworkPlugin (отправка state клиентам через UDP)

**Критерий готовности:** Server симулирует мир без GUI, клиенты подключаются и видят синхронизацию

---

## 11. Сравнение подходов (финальная таблица)

| Аспект | GDScript-primary | Rust Scripts per Node | Rust-Centric (ВЫБРАНО) |
|--------|------------------|----------------------|------------------------|
| **Логика в** | GDScript | Rust | Rust (Bevy ECS) |
| **Godot роль** | Движок + логика | Движок + scenes | Только I/O |
| **Детерминизм** | ❌ Сложно | ⚠️ Средне | ✅ Полный |
| **Headless тесты** | ❌ Нет | ⚠️ Частично | ✅ Да |
| **Rollback netcode** | ❌ Невозможен | ⚠️ Сложно | ✅ Возможен |
| **Compile time** | ✅ Мгновенный | ⚠️ Средний | ⚠️ Медленный |
| **Onboarding** | ✅ Простой | ⚠️ Средний | ⚠️ Сложный |
| **Performance** | ⚠️ Средний | ✅ Хороший | ✅ Отличный |
| **GDScript строк** | Тысячи | Сотни | 0 |

---

## 12. Ссылки и источники

### godot-rust (gdext) — 2024-2025

**Официальная документация:**
- godot-rust book: https://godot-rust.github.io/book/
- API docs: https://docs.rs/godot/latest/godot/
- GitHub: https://github.com/godot-rust/gdext

**Последние обновления:**
- May 2025 dev update: https://godot-rust.github.io/dev/may-2025-update/
  - Type-safe signals
  - OnEditor fields
  - API versioning
- June 2024 dev update: https://godot-rust.github.io/dev/june-2024-update/
  - Hot-reload improvements
  - Simplified initialization
- February 2024 dev update: https://godot-rust.github.io/dev/february-2024-update/
  - Async/await support

### godot-bevy Integration

**Библиотека:**
- GitHub: https://github.com/bytemeadow/godot-bevy
- Documentation: https://bytemeadow.github.io/godot-bevy/
- Lib.rs: https://lib.rs/crates/godot-bevy

**Ключевые фичи (v0.8+):**
- Plugin system (opt-in features)
- Transform synchronization
- Input/Audio bridges
- Bevy 0.16 + Godot 4 support

### Game Architecture Patterns

**godot-rust architecture patterns:**
- https://godot-rust.github.io/book/gdnative/overview/architecture.html
  - Rust as GDScript Extension
  - Rust Scripts for Scene Nodes
  - Rust-Centric with Godot as I/O

### Procedural Scene Building

**Tutorials:**
- Creating Scene from another Scene: https://tharinduwd.medium.com/godot-rust-creating-a-scene-from-another-scene-6133d6ec0ebe
- Godot Forum: Adding Child with Rust GDExt
  https://forum.godotengine.org/t/adding-child-with-rust-gdext/79476

### Headless Server

**Godot 4 Headless Mode:**
- Godot 4.0 Release Notes: https://godotengine.org/article/godot-4-0-sets-sail/
  - `--headless` command line argument
  - Disables rendering, window management
  - GDExtension continues working

---

## 13. Открытые вопросы

### godot-bevy vs custom bridge?

**Решение (обновлено после аудита 2025):**
- **Начать с godot-bevy** (MVP, фазы 0-3)
- **Обязательно мигрировать на custom bridge** для production (фаза 6+)

**Критерии перехода на custom (ОДИН из них = триггер):**
- ✅ **Rollback netcode активирован** — godot-bevy несовместим (ГАРАНТИРОВАННО)
- godot-bevy не поддерживает lag compensation hooks
- Performance bottleneck в автосинхронизации (>10ms на sync)
- Визуальные artifacts при rollback (teleporting entities)

**Ожидаемый timeline:**
- Фазы 1-4: godot-bevy (2-3 месяца)
- Фаза 5: rollback netcode → обнаружим конфликт → custom bridge (2 недели)
- Фаза 6+: production с custom bridge

---

### Визуальные prefab'ы: сколько делать в Godot?

**Рекомендация:** Только 3D модели + анимации + материалы
- НЕ делать: сложные иерархии с логикой
- НЕ делать: UI layouts с поведением
- Делать: простые "dumb" визуальные объекты

**Критерий:** prefab должен быть instantiate-able из Rust без дополнительной настройки

---

### Hot-reload state preservation?

**Проблема:** При перекомпиляции Rust теряется state (позиции entities, inventory)

**Решение 1 (простой):** Не сохранять state, перезапуск сцены
**Решение 2 (сложный):** Bevy snapshot → serialize → reload после recompile

**Рекомендация:** Решение 1 для MVP, Решение 2 если hot-reload станет критичным

---

## 14. Следующий шаг после прочтения этого документа

**Немедленное действие:**
1. Создать proof-of-concept: Godot + godot-rust + простой Bevy App
2. Проверить что hot-reload работает
3. Создать один visual prefab (куб) и заспавнить из Rust

**Если PoC успешен:**
1. Спроектировать GDExtension Protocol (формат данных Rust ↔ Godot)
2. Спроектировать Content Pipeline (YAML схемы для items/NPC)
3. Начать Фазу 1 из physics-architecture.md параллельно с Godot интеграцией

**Если PoC провалился:**
1. Откатиться на "Rust Scripts for Scene Nodes" подход
2. Использовать больше Godot инфраструктуры (scenes, signals)
3. Переоценить требования к детерминизму (может не критично для single-player?)

---

**Финальная рекомендация:**
Rust-Centric подход амбициозный, но технически обоснован. godot-rust (2024-2025) достаточно зрелый для production. godot-bevy библиотека даёт быстрый старт. Если команда готова к Rust learning curve — это архитектурно правильное решение для systems-driven simulation с rollback netcode требованиями.

---

## 15. Best Practices: CharacterBody3D + NavigationAgent3D

**Дата добавления:** 2025-01-10
**Источник:** Session Log "Navigation & Movement System Fix"

### ⚠️ КРИТИЧНОЕ: CharacterBody3D должен быть Root Node

**❌ АНТИПАТТЕРН (создаёт feedback loop):**
```
Node3D (root)                      ← Визуальный контейнер
└── CharacterBody3D (child)        ← Physics body
    └── CollisionShape3D
```

**✅ ПРАВИЛЬНАЯ СТРУКТУРА:**
```
CharacterBody3D (root)             ← Physics body = root node
├── CollisionShape3D
├── MeshInstance3D (визуалы)
└── NavigationAgent3D
```

### Проблема: Exponential Velocity Accumulation

**Симптомы:**
- Актор начинает двигаться всё быстрее (экспоненциальное ускорение)
- Через несколько секунд velocity достигает 50-100+ м/с вместо 2-5 м/с
- `move_and_slide()` не сбрасывает velocity между фреймами

**Root Cause:**
```rust
// ❌ ПЛОХОЙ КОД (feedback loop):
let mut parent_node = visuals.get(&entity);  // Node3D
let mut body = parent_node.get_node_as::<CharacterBody3D>("Body");

body.set_velocity(velocity);
body.move_and_slide();  // → body двигается на velocity * delta

// Синхронизация parent с child → ВОТ ПРОБЛЕМА!
parent_node.set_global_position(body.get_global_position());
```

**Что происходит:**
1. `body.move_and_slide()` двигает child CharacterBody3D на `velocity * delta`
2. `parent_node.set_global_position()` двигает parent Node3D → child автоматически двигается **ЕЩЁ РАЗ** (relative transform сохраняется)
3. Следующий фрейм: `body.get_velocity()` содержит **удвоенную velocity** → экспоненциальный рост
4. Результат: актор улетает в космос 🚀

**Решение:**
```rust
// ✅ ХОРОШИЙ КОД (root = CharacterBody3D):
let actor_node = visuals.get(&entity);
let mut body = actor_node.cast::<CharacterBody3D>();  // Root сам physics body

let velocity = Vector3::new(
    direction.x * MOVE_SPEED,
    body.get_velocity().y,  // Сохраняем gravity
    direction.z * MOVE_SPEED,
);

body.set_velocity(velocity);
body.move_and_slide();
// НЕТ синхронизации — root node сам двигается через move_and_slide()!
```

### Правила для TSCN Prefabs с Physics

1. **Root node ВСЕГДА physics node** (CharacterBody3D, RigidBody3D)
2. **НЕ оборачивать physics body в Node3D/Node wrapper**
3. **НЕ синхронизировать parent ↔ child physics positions**
4. **Velocity перезаписывать полностью** (не incrementally):
   ```rust
   // ✅ ПРАВИЛЬНО:
   body.set_velocity(new_velocity);  // Полная замена

   // ❌ НЕПРАВИЛЬНО:
   let old = body.get_velocity();
   body.set_velocity(old + delta_velocity);  // Накопление!
   ```

5. **Сохранять Y компонент для gravity:**
   ```rust
   let velocity = Vector3::new(
       horizontal.x * speed,
       body.get_velocity().y,  // ← КРИТИЧНО для gravity
       horizontal.z * speed,
   );
   ```

### Диагностика проблем с velocity

Добавить лог перед/после `move_and_slide()`:
```rust
let old_vel = body.get_velocity();
voidrun_simulation::log(&format!("velocity BEFORE: {:?}", old_vel));

body.set_velocity(new_velocity);
body.move_and_slide();

let final_vel = body.get_velocity();
voidrun_simulation::log(&format!("velocity AFTER: {:?}", final_vel));
```

**Если `old_vel != new_velocity` (не равны предыдущему set значению)** → есть feedback loop или накопление.

### Ссылки

- **Session Log:** `docs/sessions/2025-01-10-navigation-movement-fix.md`
- **Fixed Code:** `crates/voidrun_godot/src/systems/movement_system.rs`
- **Fixed Prefab:** `godot/actors/test_actor.tscn`
