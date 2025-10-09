# VOIDRUN: Bevy ECS Architecture Design Document

## Версия: 2.0 (обновлено 2025-10-07)
## Bevy Version: **0.16** (апрель 2025)

## 1. Общая позиция по Bevy ECS

**Рекомендация:** Использовать Bevy ECS как ядро симуляционного движка с **гибридной архитектурой** — таблицы для hot-path компонентов (позиции, здоровье, AI state), sparse sets для редких эффектов (временные баффы, квестовые маркеры).

**Почему Bevy ECS подходит для VOIDRUN:**
- **Нативная параллелизация:** автоматический параллельный executor с разрешением зависимостей
- **Каденс-плюрализм:** встроенная поддержка FixedUpdate (физика/AI) + Update (рендер/UI) + кастомные schedule с run conditions
- **Event-driven связи:** встроенная шина событий (Events<T>) с двухфреймовым буфером — идеально для междоменных взаимодействий
- **Headless симуляция:** официальная поддержка no-render режима для CI-тестов
- **Snapshot/rollback ecosystem:** готовые crates (bevy_save, bevy_snap) для сохранений и откатов
- **ECS Relationships (0.16):** встроенные one-to-many связи для hitbox иерархий
- **Required Components (0.15+):** композиция без Bundles — меньше boilerplate

---

## 2. Ключевые механики Bevy ECS для VOIDRUN

### 2.1 Каденс-разделение через Multiple Schedules

**Проблема:** Физика 60-120Hz, AI 0.5-1Hz, экономика 0.1-0.2Hz, квесты event-driven.

**Решение Bevy:**
- **FixedUpdate** (Fixed timestep, default 64Hz): физика, боевая механика, stamina decay
- **Update** (Per-frame): рендер, UI, визуальные эффекты, интерполяция
- **Кастомные schedule** с run conditions:
  - `EconomySchedule.run_if(on_timer(5.0))` — экономика раз в 5 секунд
  - `AISchedule.run_if(on_timer(1.0))` — AI обновление раз в секунду
  - `QuestSchedule` триггерится через события (OnAdd, OnRemove observers)

**Trade-offs:**
- ✅ Нативное разделение по частотам без костылей
- ✅ Детерминизм в FixedUpdate: одинаковое количество шагов при одинаковом времени
- ⚠️ Фазовые сдвиги между schedule: событие из AI может придти в экономику с задержкой N тиков
- ⚠️ Нужны инварианты: "квест не требует данные из будущего" (проверяем ассертами)

**Инвариант для CI:**
```
После N тиков FixedUpdate:
  - Все события обработаны (queues пусты)
  - Все цены неотрицательны
  - Все квестовые граф-рёбра DAG
```

---

### 2.2 Snapshot → Systems → Diffs → Atomic Apply через Commands + Exclusive Systems

**Проблема:** Читаем из снепшота, пишем через diff, применяем атомарно в конце тика.

**Решение Bevy:**
1. **Чтение (Query):** системы читают компоненты/ресурсы через `Query<&T>`, `Res<T>` — это снепшот начала тика
2. **Запись (Commands):** отложенные мутации через `Commands` (spawn, insert, remove, trigger events)
3. **Apply точка:** автоматический `ApplyDeferred` между системами с зависимостями (`.before()`, `.after()`)
4. **Exclusive system:** для атомарных операций на `&mut World` — применяется мгновенно

**Trade-offs:**
- ✅ Change detection: автоматическое отслеживание Changed<T>, Added<T> через monotonic tick counters
- ✅ Параллелизм: чтение параллельно, запись отложенно → нет блокировок
- ✅ Детерминизм: фиксированный порядок apply через explicit ordering
- ⚠️ Задержка на 1 sync point: `Commands` видны другим системам только после `ApplyDeferred`
- ⚠️ Exclusive systems блокируют параллелизм — использовать **только для критичных атомарных операций** (применение экономических diff'ов, применение квестовых флагов)

**Паттерн для VOIDRUN:**
```
Физика/AI/Экономика системы (параллельно):
  - Query<&Position, &Velocity> для чтения
  - Commands::insert(Damage) для записи

ApplyDeferred (автоматически)

Exclusive атомарный апплаер:
  - &mut World для применения накопленных diff'ов из ресурса DiffBuffer
  - World::flush() для применения хуковых команд
```

---

### 2.3 Event Bus через Events<T> + Observers

**Проблема:** Междоменные связи без прямых зависимостей (AI → Economy → Quests).

**Решение Bevy:**
- **Events<T> resource:** глобальная шина событий с двухфреймовым буфером
- **EventWriter/EventReader:** pub/sub паттерн
- **Observers (механизм 0.14+, изменения в 0.16):** reactive системы, автоматически запускаются при триггере события

**⚠️ КРИТИЧЕСКОЕ ИЗМЕНЕНИЕ В 0.16:**
- Порядок выполнения изменён: **Observers запускаются ДО hooks** (было наоборот)
- **Lifecycle:** `add → hooks → observers` стало `add → observers → hooks`
- **Для on_remove/on_replace:** observers видят entity ДО cleanup hooks
- **Риск:** если hook удаляет данные, observer может увидеть некорректное состояние
- **Решение:** использовать `Commands::trigger` (отложенный) вместо `World::trigger` (мгновенный)

**Ключевые события VOIDRUN:**
- `ShipDestroyed { entity, faction }`
- `PriceChanged { sector, item, old, new }`
- `QuestAdvanced { quest_id, stage }`
- `WarDeclared { faction_a, faction_b }`
- `ReputationChanged { player, faction, delta }`

**Trade-offs:**
- ✅ Двухфреймовый буфер: события живут минимум 2 кадра → защита от race conditions
- ✅ Observers: lifecycle events (OnAdd<Mine>, OnRemove<Health>) → реактивная логика без ручного polling
- ⚠️ События теряются, если читатель не запущен: квесты в pause state пропустят события (решение: custom cleanup + state scoped events)
- ⚠️ Порядок обработки: если sender и reader в одном тике — нужен explicit `.before()/.after()`

**Паттерн VOIDRUN:**
```
Domain A (AI):
  EventWriter<ShipDestroyed> на конце AI schedule

Domain B (Economy):
  EventReader<ShipDestroyed> в начале Economy schedule → обновляет cargo supply

Domain C (Quests):
  Observer<OnRemove<Enemy>> → триггерит QuestAdvanced автоматически
```

---

### 2.4 Детерминизм для Saves/Replays через bevy_save + Controlled RNG

**Проблема:** Сохранения должны быть bit-perfect reproducible, реплеи должны converge.

**Решение Bevy:**
- **bevy_save crate:** Snapshot API для capture/restore всех ресурсов и компонентов
- **WorldCheckpointExt:** `World::checkpoint()` → snapshot, `World::rollback(t)` → откат к моменту времени
- **Детерминизм FixedUpdate:** одинаковый timestep → одинаковое количество итераций
- **Controlled RNG:** `Resource<RngState>` с seed, change detection для проверки "RNG использован в правильный момент"

**Trade-offs:**
- ✅ Snapshot: `World::capture()` сериализует весь мир в JSON/RON (поддержка версионирования через `bevy_save`)
- ✅ Rollback: готовый механизм для rewind (годится для турновых реплеев, network rollback netcode)
- ⚠️ **Критический риск:** системы без explicit ordering легко ломают детерминизм → обязательно `.chain()` для критичных path'ов
- ⚠️ Sparse set components могут менять порядок итерации → для детерминизма **только table storage** в критичных системах
- ⚠️ `Entity` — это pointer, не ID: для save/load/network нужен свой `StableId` компонент (u64 или UUID)

**Инварианты для CI:**
```rust
// Property test:
let world_before = capture();
let cmds = record_inputs();  // Записываем все Commands
world.apply(cmds.clone());
let world_after = capture();

// Replay:
let world_replay = restore(world_before);
world_replay.apply(cmds);
assert_eq!(capture(world_replay), world_after);  // Bit-perfect match
```

---

### 2.5 Headless симуляция для CI через Minimal Plugins

**Проблема:** Галактические прогоны 500h без GPU, проверка инвариантов в CI.

**Решение Bevy:**
```rust
App::new()
    .add_plugins(MinimalPlugins)  // Без WinitPlugin, без RenderPlugin
    .add_plugins(SimulationPlugins)  // Только ECS, физика, AI, экономика
    .run_headless(ticks=1_000_000);  // 500h при 60Hz = 108M тиков
```

**Trade-offs:**
- ✅ Zero overhead: нет рендера, нет окна, нет ввода → pure ECS + logic
- ✅ Параллельные тесты: headless App изолированы, можно запускать 100 параллельных симов
- ✅ Deterministic CI: фиксированный timestep + controlled RNG → reproducible результаты
- ⚠️ Требуется четкое разделение: SimulationPlugin не должен зависеть от RenderPlugin, InputPlugin
- ⚠️ Events должны корректно работать без рендера: UI события нужно мокать в тестах

**Паттерн для VOIDRUN:**
```rust
// Production:
App::new()
    .add_plugins(DefaultPlugins)  // Рендер, окно, звук
    .add_plugins(SimulationPlugin)
    .add_plugins(UIPlugin)

// CI tests:
App::new()
    .add_plugins(MinimalPlugins)
    .add_plugins(SimulationPlugin)  // Тот же код!
    .insert_resource(TestConfig { ticks: 1M })
    .add_systems(Last, assert_invariants)
```

---

### 2.6 Component Storage Strategy: Table vs Sparse Set

**Проблема:** Оптимизировать cache-friendly iteration vs частые add/remove.

**Решение Bevy:**
- **Table storage (default):** для hot-path компонентов, которые итерируются часто
  - Примеры: `Position`, `Velocity`, `Health`, `AIState`, `FactionId`
  - Итерация в 2-4x быстрее благодаря contiguous memory
- **Sparse set storage:** для редких, временных компонентов
  - Примеры: `Stunned`, `Burning`, `QuestMarker`, `TradeDealPending`
  - Добавление/удаление O(1), но итерация медленнее

**Trade-offs:**
- ✅ Гибридная стратегия: горячие данные в таблицах, эффекты в sparse sets
- ✅ Sparse sets не фрагментируют таблицы: можно добавлять/удалять эффекты без переаллокации всей archetypes
- ⚠️ Нужно профилировать: неправильный выбор может убить performance (query с миксом table + sparse принудительно sparse)
- ⚠️ Детерминизм: sparse set iteration не гарантирует порядок → для reproducible итерации **только table storage**

**Правило для VOIDRUN:**
```
Table storage:
  - Все базовые компоненты (Transform, Health, Faction, Inventory)
  - Компоненты, по которым Query каждый тик

Sparse set storage:
  - Временные эффекты (Buffs, Debuffs, Status effects)
  - Квестовые маркеры (QuestTarget, DialogueAvailable)
  - Компоненты, добавляемые/удаляемые часто, итерируемые редко
```

---

### 2.7 Plugin Architecture для модульности

**Проблема:** 7 доменов (Physics, AI, Economy, Quests, Space, Survival, UI) — как избежать god-plugin?

**Решение Bevy:**
- **1 Plugin = 1 домен:** `PhysicsPlugin`, `AIPlugin`, `EconomyPlugin`, etc.
- **System Sets для ordering:** каждый Plugin экспортирует `pub enum PhysicsSet { Collisions, Movement }`
- **События для связей:** плагины общаются **только через Events**, не через прямые вызовы
- **Ресурсы для конфига:** immutable `Res<EconomyConfig>`, mutable runtime state `ResMut<MarketState>`
- **ECS Relationships (0.16):** используем для parent-child иерархий (hitbox trees, ship modules)

**Trade-offs:**
- ✅ Модульность: можно отключить UIPlugin для headless, можно подменить PhysicsPlugin на моковый
- ✅ Decoupling: AI не знает про Economy, только про `Events<PriceChanged>`
- ✅ Переиспользование: можно публиковать отдельные плагины (e.g., `voidrun_economy` crate)
- ⚠️ System sets per-schedule: если забыли сконфигурировать set в нужном schedule — silent fail (компилируется, но ordering не работает)
- ⚠️ Overhead организация: больше boilerplate (каждый plugin = файл + структура + impl Plugin)

**Структура VOIDRUN:**
```
crates/
  voidrun_core/     // MinimalPlugins + SharedTypes (Entity, Events definitions)
  voidrun_physics/  // PhysicsPlugin: FixedUpdate системы, CollisionEvents
  voidrun_ai/       // AIPlugin: AISchedule (1Hz), FactionSystems, DecisionTrees
  voidrun_economy/  // EconomyPlugin: EconomySchedule (0.2Hz), MarketSystems
  voidrun_quests/   // QuestsPlugin: Observers (OnAdd/OnRemove), QuestStateMachines
  voidrun_space/    // SpaceFlightPlugin: ShipPhysics, DogfightAI
  voidrun_survival/ // SurvivalPlugin: Hunger, O2, Repair
  voidrun_client/   // UIPlugin: Update schedule, EventReaders для отображения
```

---

### 2.8 Observers + Hooks для реактивной логики

**Проблема:** "Когда враг умирает → обновить квест" — polling или reactive?

**Решение Bevy (0.14+, обновлено 0.16):**
- **Observers:** системы, автоматически запускаемые при событии
  - `World::trigger(event)` → мгновенно запускает все observers
  - `Commands::trigger(event)` → запускает в следующий sync point
- **Component Hooks:** lifecycle callbacks при add/remove компонента
  - **Порядок 0.16:** `add → observers → hooks`; `remove → observers → hooks`
  - **Изменено с 0.15:** hooks теперь primitive (первые и последние), observers между ними

**⚠️ BREAKING CHANGE 0.16:**
- Observers теперь видят entity **ДО** cleanup hooks
- Если hook удаляет данные (например, `remove_child()`), observer может паниковать при доступе
- **Рекомендация для VOIDRUN:** всегда проверять `entity.is_despawned()` в observers или использовать `Commands::trigger`

**Примеры VOIDRUN:**
```rust
// Квест реагирует на смерть врага:
world.observe(|trigger: Trigger<OnRemove, Enemy>, mut quests: Query<&mut QuestProgress>| {
    let enemy_entity = trigger.entity();
    // Обновить квест без polling
});

// Автоматическая cleanup при удалении корабля:
world.component_hooks::<Ship>()
    .on_remove(|world, entity, _| {
        // Освободить cargo, удалить trade deals
    });
```

**Trade-offs:**
- ✅ Реактивность: нет необходимости в системах-сканерах "проверить все квесты каждый тик"
- ✅ Локальность: логика квеста рядом с событием, а не разбросана по системам
- ⚠️ Порядок выполнения: observers триггерятся немедленно → могут нарушить expected ordering (решение: Commands::trigger для отложенного запуска)
- ⚠️ Hooks не должны паниковать: panic в hook убивает весь процесс (решение: unwrap_or_log)

---

### 2.9 Change Detection для diff'ов

**Проблема:** Отправлять по сети/в UI только изменения, не весь мир.

**Решение Bevy:**
- **Changed<T>:** Query фильтр для компонентов, изменённых с прошлого запуска системы
- **Added<T>:** для новых компонентов
- **Monotonic tick counters:** каждое изменение увеличивает tick, Query отслеживает last_seen_tick

**Примеры VOIDRUN:**
```rust
// Отправить клиенту только изменённые позиции:
fn sync_positions(query: Query<(Entity, &Position), Changed<Position>>, mut net: ResMut<NetState>) {
    for (entity, pos) in query.iter() {
        net.send(PositionUpdate { entity: entity.to_stable_id(), pos });
    }
}

// UI обновляет цены только при изменении:
fn update_ui_prices(query: Query<(&Item, &Price), Changed<Price>>, mut ui: ResMut<UIState>) {
    for (item, price) in query.iter() {
        ui.update_price(item, price);
    }
}
```

**Trade-offs:**
- ✅ Эффективность: не сканируем весь мир, только Changed
- ✅ Автоматика: change detection работает из коробки, не нужно руками трекать dirty flags
- ⚠️ False positives: `.bypass_change_detection()` для внутренних мутаций, иначе всё помечается Changed
- ⚠️ Per-system state: каждая система отслеживает свой last_tick → если система долго не запускалась, увидит "всё изменилось"

---

## 3. Риски и антипаттерны

### 3.1 Детерминизм легко сломать
**Симптом:** Saves загружаются с другим состоянием, replays desync.

**Причины:**
- Неявный порядок систем (параллельный executor может менять порядок)
- Sparse set iteration (порядок не гарантирован)
- Недетерминированный RNG (SystemTime::now(), rand::thread_rng())
- Float операции (разные результаты на разных CPU — нужен fixed-point или OrderedFloat)

**Обнаружение:**
- Property test: `save → load → save` должен дать bit-identical результат
- CI replay test: записать inputs, переиграть 100 раз → все результаты одинаковы
- Asserts: `debug_assert!(world.resource::<RngState>().used_this_tick == false)` в начале critical систем

---

### 3.2 Фазовые сдвиги между schedule
**Симптом:** AI принял решение на основе старых цен, квест не видит новую репутацию.

**Причины:**
- EconomySchedule (0.2Hz) и AISchedule (1Hz) не синхронизованы
- События из медленного schedule приходят в быстрый с задержкой

**Решение:**
- **Явные гарантии:** "AI принимает решения на основе цен, актуальных на момент прошлого запуска EconomySchedule"
- **Инварианты:** "Квест не требует данные из будущего" — валидатор в CI
- **Debounce:** события из медленных schedule буферизовать и отправлять батчами

---

### 3.3 Entity ID не переживает save/load/network
**Симптом:** После загрузки save Entity ID'ы другие, квесты потеряли ссылки на NPC.

**Причины:**
- `Entity` — это pointer (archetype + index), не stable ID
- При загрузке bevy_save генерирует новые Entity

**Решение:**
- **StableId компонент:** `Component { stable_id: u64 }` — уникальный persistent ID
- **Registry:** `Resource<StableIdRegistry>: HashMap<u64, Entity>` для lookup
- **Квесты хранят StableId:** `QuestTarget { npc_id: u64 }` вместо `Entity`

---

### 3.4 Exclusive systems убивают параллелизм
**Симптом:** Профайлер показывает idle треды, frame time высокий.

**Причины:**
- Слишком много `&mut World` систем
- Exclusive system в середине schedule блокирует параллелизм

**Решение:**
- **Минимизировать:** использовать Exclusive только для атомарных операций (apply diff'ов)
- **Группировать:** все Exclusive в конец schedule, чтобы параллельные отработали первыми
- **Альтернатива:** Commands + ApplyDeferred вместо Exclusive (отложенное применение)

---

### 3.5 Event loss в pause/inactive states
**Симптом:** После возврата из паузы квесты не обновились, UI показывает старые цены.

**Причины:**
- События живут 2 фрейма, если читатель не запущен — потеряются
- State scoped systems не получают события из других states

**Решение:**
- **Custom event cleanup:** вместо 2-frame buffer — хранить до явной clear
- **Event replay:** при возврате в state переиграть пропущенные события из лога
- **State-agnostic readers:** критичные EventReader в Update, а не в State-specific schedule

---

## 4. План внедрения (пошагово)

### Фаза 1: Минимальный ECS Foundation (1-2 недели)
1. **Создать структуру плагинов:** CorePlugin (Minimal), PhysicsPlugin (FixedUpdate)
2. **Настроить schedules:** FixedUpdate (64Hz), Update, PhysicsSchedule
3. **Базовые компоненты:** Position, Velocity, Health (table storage)
4. **Тест:** headless прогон 1000 тиков, assert(entities alive)

**Критерий готовности:** CI прогоняет headless симуляцию 1000 тиков без паники.

---

### Фаза 2: Event Bus + Interdomain Communication (1 неделя)
1. **Определить ключевые события:** ShipDestroyed, PriceChanged, QuestAdvanced
2. **Имплементировать pub/sub:** EventWriter в PhysicsPlugin, EventReader в future EconomyPlugin (stub)
3. **Тест:** триггерить ShipDestroyed → проверить, что событие прочитано в следующем тике

**Критерий готовности:** События проходят между плагинами, двухфреймовый буфер работает корректно.

---

### Фаза 3: Snapshot/Rollback для Saves (1-2 недели)
1. **Интегрировать bevy_save:** добавить в Cargo.toml, пометить сериализуемые компоненты
2. **Имплементировать StableId:** компонент + registry, маппинг Entity ↔ u64
3. **Тест:** `world → save → load → save` → сравнить файлы (bit-identical)
4. **Property test:** записать 100 inputs, replay 10 раз → одинаковый результат

**Критерий готовности:** CI прогоняет property test с 100% reproducibility.

---

### Фаза 4: Детерминизм + Controlled RNG (1 неделя)
1. **Resource<RngState>:** seeded RNG, change detection для debug
2. **Explicit ordering:** `.chain()` для критичных систем (physics, AI decisions)
3. **Float handling:** заменить f32 на OrderedFloat или fixed-point в критичных местах
4. **Тест:** parallel CI запуск — 10 симов с одинаковым seed → bit-identical результаты

**Критерий готовности:** 10 параллельных headless прогонов дают identical snapshots.

---

### Фаза 5: Observers + Reactive Quests (1 неделя)
1. **Квестовый Observer:** OnRemove<Enemy> → QuestAdvanced trigger
2. **Hooks для cleanup:** Ship::on_remove → освободить cargo
3. **Тест:** убить врага → проверить, что квест обновился в том же тике

**Критерий готовности:** Observers реагируют на lifecycle events, квесты обновляются реактивно.

---

### Фаза 6: Мультидоменные плагины + каденсы (2 недели)
1. **EconomyPlugin:** EconomySchedule (0.2Hz), MarketSystems
2. **AIPlugin:** AISchedule (1Hz), DecisionTrees
3. **Связать через события:** AI читает PriceChanged, Economy читает ShipDestroyed
4. **Тест:** 500h headless прогон, проверить инварианты (цены ≥ 0, AI state valid)

**Критерий готовности:** Headless прогон 500h без паники, все инварианты зелёные.

---

### Фаза 7: Headless CI + Long-Run Tests (ongoing)
1. **CI pipeline:** headless прогон на каждый PR (1M тиков)
2. **Invariant checks:** assert цен, репутации, квестовых графов
3. **Benchmarks:** bevy_benchmark_games для отслеживания performance regression

**Критерий готовности:** CI зелёный, benchmarks в приемлемых пределах (p95 < 16ms/tick для 64Hz).

---

## 5. Контракты и данные (текстовое описание)

### 5.1 Базовые компоненты (table storage)
```
Position { x, y, z: f64 }  // Fixed-point или OrderedFloat для детерминизма
Velocity { dx, dy, dz: f64 }
Health { current, max: u32 }
Stamina { current, max: u32 }
FactionId { id: u64 }  // StableId для фракций
AIState { state: enum, target: Option<u64> }  // StableId для target
Ship { hull, shields, energy: u32 }
Inventory { items: Vec<(ItemId, u32)> }
```

### 5.2 Временные эффекты (sparse set storage)
```
Stunned { duration: f32 }
Burning { damage_per_sec: u32, duration: f32 }
QuestMarker { quest_id: u64 }
TradeDealPending { trader_id: u64, item: ItemId, amount: u32 }
```

### 5.3 Ресурсы (global singleton)
```
RngState { rng: ChaCha8Rng, seed: u64, used_this_tick: bool }
MarketState { prices: HashMap<(SectorId, ItemId), u32> }
FactionRelations { reputation: HashMap<(FactionId, FactionId), i32> }
QuestRegistry { active: Vec<QuestState>, completed: HashSet<u64> }
StableIdRegistry { next_id: u64, entity_map: HashMap<u64, Entity> }
```

### 5.4 События (pub/sub)
```
ShipDestroyed { entity_id: u64, faction: u64, killer: Option<u64> }
PriceChanged { sector: u64, item: ItemId, old: u32, new: u32 }
QuestAdvanced { quest_id: u64, old_stage: u32, new_stage: u32 }
WarDeclared { faction_a: u64, faction_b: u64 }
ReputationChanged { player: u64, faction: u64, delta: i32 }
DamageDealt { attacker: u64, target: u64, amount: u32, weapon_type: enum }
```

---

## 6. Метрики и профилирование

### Целевые метрики:
- **FixedUpdate:** p95 < 8ms (для 120Hz), p99 < 12ms
- **Update:** p95 < 16ms (для 60fps), p99 < 33ms
- **EconomySchedule:** p95 < 100ms (0.2Hz = 5sec period → budget 100ms)
- **Memory:** ≤ 2GB для 100K entities, ≤ 8GB для 1M entities

### Профилировать:
- `tracy_client` интеграция для per-system timing
- `bevy diagnostic` плагин для FPS/entity count
- Custom assertions: `assert!(system_time < threshold)` в headless прогонах

---

## 7. Открытые вопросы для решения

### 7.1 Детерминизм float операций
**Вопрос:** Использовать fixed-point арифметику (bevy_fixed) или OrderedFloat для физики/экономики?

**Trade-offs:**
- **Fixed-point:** 100% детерминизм, но сложнее отладка, ограниченный range
- **OrderedFloat:** проще в use, но всё равно могут быть различия на разных CPU (FMA instructions)

**Рекомендация:** Начать с OrderedFloat для прототипа, если увидим desync — мигрировать критичные части на fixed-point.

---

### 7.2 Netcode архитектура
**Вопрос:** Snapshot-interpolation (классический client-server) или rollback (peer-to-peer GGRS)?

**Trade-offs:**
- **Snapshot-interpolation:** проще реализация, server authoritative, но больше latency
- **Rollback (GGRS):** instant response, но требует bit-perfect determinism и больше CPU

**Рекомендация:** Для кооператива (2-4 игрока) — rollback через GGRS, т.к. VOIDRUN уже строится с детерминизмом в уме. Для MMO (будущее) — snapshot-interpolation.

---

### 7.3 Моддинг API
**Вопрос:** Scripting layer (Lua/WASM) или чистый Rust API для модов?

**Trade-offs:**
- **Lua/WASM:** безопасность (sandbox), проще для моддеров, но overhead и ограничения
- **Rust API:** полная мощь ECS, zero overhead, но требует recompile и может крашнуть игру

**Рекомендация:** MVP — Rust API (plugins как моды), после 1.0 — добавить WASM runtime для безопасных user-generated контента.

---

### 7.4 Godot интеграция глубина
**Вопрос:** Godot как thin client (только рендер+input) или thick client (часть UI логики)?

**Trade-offs:**
- **Thin client:** вся логика в Rust, Godot только визуализирует → проще headless testing, но больше network traffic
- **Thick client:** часть UI logic в GDScript → меньше latency для UI, но сложнее синхронизация

**Рекомендация:** Thin client для первой версии (вся логика в Rust ECS), UI state через Change Detection → Godot просто рисует. Если увидим bottleneck — переносить UI state machine в Godot.

---

## 8. Следующие шаги

1. **Утвердить ответы на открытые вопросы** (7.1-7.4)
2. **Создать ADR-001:** "Bevy ECS как core simulation engine"
3. **Запустить Фазу 1:** минимальный ECS foundation
4. **Настроить CI:** headless test infrastructure

---

**Дата создания:** 2025-10-07
**Последнее обновление:** 2025-10-07 (аудит 2024-2025)
**Версия:** 2.0
**Bevy Version:** 0.16 (апрель 2025)
**Статус:** Validated (проверено против актуальных best practices)
