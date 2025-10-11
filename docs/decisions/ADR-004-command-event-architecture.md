# ADR-004: Command/Event Architecture (Bevy Events)

**Дата:** 2025-01-10
**Статус:** ✅ ПРИНЯТО
**Связанные ADR:** [ADR-002](ADR-002-godot-rust-integration-pattern.md), [ADR-003](ADR-003-ecs-vs-godot-physics-ownership.md)

## Контекст

После принятия **ADR-003 (Hybrid Architecture)** необходимо определить механизм обмена данными между ECS (voidrun_simulation) и Godot (voidrun_godot).

### Проблема

ECS и Godot живут в разных слоях:
- **ECS (Strategic Layer):** Authoritative game state, AI decisions, combat rules
- **Godot (Tactical Layer):** Transform, physics, animations, visual effects

Необходимо:
1. **ECS → Godot:** "Сыграй анимацию атаки", "Спавни визуал для entity"
2. **Godot → ECS:** "Анимация закончилась", "Weapon hitbox коллизия", "Игрок нажал WASD"

### Рассмотренные варианты

#### Вариант A: Trait-based (GodotBridge)

```rust
pub trait GodotBridge {
    fn spawn_visual(&mut self, entity: Entity, prefab: &str, pos: Vec3);
    fn play_animation(&mut self, entity: Entity, anim: &str);
}

// Godot реализует trait
impl GodotBridge for GodotBridgeImpl { ... }
```

**Проблема:** Это **реинвент PresentationClient** (который отложили в ADR-002 по YAGNI).

#### Вариант B: Custom Command/Event queues с handlers

```rust
trait SimulationCommand: Send + Sync + 'static {
    fn execute(&self, world: &mut World);
}

struct CommandQueue {
    commands: Vec<Box<dyn SimulationCommand>>,
}
```

**Проблема:** Громоздко, требует boilerplate для каждого модуля.

#### Вариант C: Bevy Events напрямую (KISS) ← **ВЫБРАН**

```rust
#[derive(Event, Clone, Debug)]
pub enum GodotInputEvent {
    WeaponHit { attacker: Entity, victim: Entity },
    AnimationFinished { entity: Entity, animation: String },
}

// Godot отправляет
world.send_event(GodotInputEvent::WeaponHit { ... });

// ECS читает
fn process_hits(mut events: EventReader<GodotInputEvent>) { ... }
```

**Почему:** Bevy Events — встроенный механизм, zero boilerplate, модульность через типы.

## Решение

**Прямая зависимость voidrun_godot → voidrun_simulation через Bevy Events**

### Архитектура

```
┌─────────────────────────────────────┐
│ voidrun_simulation (ECS)            │
│                                     │
│ - Определяет Event типы             │
│ - Экспортирует компоненты           │
│ - Системы читают EventReader<T>     │
└─────────────────────────────────────┘
         ↑ reads/writes    ↑ sends events
         │ (direct import) │
┌─────────────────────────────────────┐
│ voidrun_godot (Visualization)       │
│                                     │
│ - Импортирует simulation crate      │
│ - Отправляет события через          │
│   World::send_event()               │
│ - Читает ECS state через            │
│   Changed<T> queries                │
└─────────────────────────────────────┘
```

**Ключевое:** Tight coupling — это **ОК**, потому что Godot = единственный клиент (YAGNI из ADR-002).

### Event Types

#### Domain Events — модульные события из Godot в ECS

**ОБНОВЛЕНО (2025-01-10):** Вместо одного `GodotInputEvent` используем **domain-specific events** для лучшей модульности.

**Почему разделение:**
- ✅ Чёткие domain boundaries (combat, AI, transform, animation)
- ✅ Системы подписываются только на нужные события
- ✅ Bevy может планировать системы параллельно (разные Event типы)
- ✅ Легче тестировать (каждый домен изолирован)

```rust
// === voidrun_simulation/src/events/combat.rs ===

#[derive(Event, Clone, Debug)]
pub enum GodotCombatEvent {
    /// Weapon hitbox (Area3D) коллизия с entity
    WeaponHit {
        attacker: Entity,
        victim: Entity,
        hitbox_name: String, // "sword_blade", "axe_head"
    },

    /// Parry успешно сработал
    Parry {
        defender: Entity,
        attacker: Entity,
    },
}

// === voidrun_simulation/src/events/animation.rs ===

#[derive(Event, Clone, Debug)]
pub enum GodotAnimationEvent {
    /// AnimationPlayer завершил анимацию
    AnimationFinished {
        entity: Entity,
        animation: String, // "attack_swing", "dodge_roll"
    },

    /// Animation trigger (custom AnimationPlayer event)
    AnimationTrigger {
        entity: Entity,
        trigger_name: String, // "footstep", "weapon_trail_start"
    },
}

// === voidrun_simulation/src/events/transform.rs ===

#[derive(Event, Clone, Debug)]
pub enum GodotTransformEvent {
    /// Entity пересёк границу чанка (zone transition)
    ZoneTransition {
        entity: Entity,
        new_chunk: ChunkCoord, // IVec2
    },

    /// Godot отправляет точную позицию после spawn (для детерминистичных saves)
    PostSpawn {
        entity: Entity,
        actual_position: Vec3,
    },

    /// NavigationAgent3D достиг target_position
    ArrivedAtDestination {
        entity: Entity,
    },
}

// === voidrun_simulation/src/events/ai.rs ===

#[derive(Event, Clone, Debug)]
pub enum GodotAIEvent {
    /// Vision cone (Area3D) обнаружил target
    ActorSpotted {
        observer: Entity,
        target: Entity,
    },

    /// Vision cone потерял target
    ActorLost {
        observer: Entity,
        target: Entity,
    },
}

// === voidrun_simulation/src/events/input.rs ===

#[derive(Event, Clone, Debug)]
pub struct PlayerInputEvent {
    pub movement: Vec3,
    pub look_dir: Vec3,
    pub jump: bool,
    pub dodge: bool,
}
```

#### Domain Events — модульные события внутри ECS

Каждый модуль (combat, ai, economy) определяет **свои** типы событий:

```rust
// === voidrun_simulation/src/combat/events.rs ===

#[derive(Event, Debug, Clone)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub victim: Entity,
    pub amount: f32,
    pub damage_type: DamageType,
}

#[derive(Event, Debug, Clone)]
pub struct EntityDied {
    pub entity: Entity,
    pub killer: Option<Entity>,
}

// === voidrun_simulation/src/ai/events.rs ===

#[derive(Event, Debug, Clone)]
pub struct ZoneTransitionEvent {
    pub entity: Entity,
    pub from: ChunkCoord,
    pub to: ChunkCoord,
}

#[derive(Event, Debug, Clone)]
pub struct AIGoalChanged {
    pub entity: Entity,
    pub old_goal: AIGoal,
    pub new_goal: AIGoal,
}
```

**Модульность через Bevy Event Bus:**
- Каждый модуль регистрирует свои события
- Системы подписываются через `EventReader<T>`
- Pub/Sub без центрального enum

### Sync механизм: Change Detection + NonSend Resources

**Godot sync системы** используют **Bevy Change Detection** для отслеживания изменений компонентов:

```rust
// === voidrun_godot/src/animation_sync.rs ===

use voidrun_simulation::ai::AIState;
use godot::prelude::*;

/// Sync AI state → Godot AnimationPlayer
///
/// NAMING CONVENTION: `_main_thread` суффикс = система использует Godot API (NonSend resources)
/// Bevy автоматически запускает в main thread через NonSend<T> parameter.
pub fn sync_ai_animations_main_thread(
    // Changed<AIState> — только entity где AIState изменился с прошлого кадра
    query: Query<(Entity, &AIState), Changed<AIState>>,
    visuals: NonSend<VisualRegistry>, // NonSend → main thread only
) {
    for (entity, state) in query.iter() {
        // Получить Godot ноду
        if let Some(node) = visuals.visuals.get(entity) {
            let mut anim_player = node.get_node_as::<AnimationPlayer>("AnimationPlayer");

            // Проиграть анимацию в зависимости от state
            match state {
                AIState::Idle => anim_player.play("idle".into()),
                AIState::Chasing { .. } => anim_player.play("run".into()),
                AIState::Attacking => anim_player.play("attack_swing".into()),
                AIState::Fleeing => anim_player.play("run_backward".into()),
            }
        }
    }
}
```

**КРИТИЧЕСКИ ВАЖНО: Main Thread Safety**

**Проблема:** `Gd<Node3D>` содержит `*mut` указатель и **НЕ** `Send + Sync`. Godot Scene Tree API — single-threaded, thread-unsafe.

**Решение:** `NonSend<T>` / `NonSendMut<T>` resources

```rust
// Регистрация NonSend resource
app.insert_non_send_resource(VisualRegistry::default());

// Bevy автоматически определяет что система требует main thread (через NonSend parameter)
app.add_systems(Update, sync_ai_animations_main_thread);
```

**Правила для систем с Godot API:**

1. **ОБЯЗАТЕЛЬНЫЙ `_main_thread` суффикс в имени функции:**
```rust
/// Sync health → Godot Label3D
pub fn sync_health_labels_main_thread(
    query: Query<(Entity, &Health), Changed<Health>>,
    visuals: NonSend<VisualRegistry>,
) { ... }

/// Spawn visual для нового Actor
pub fn spawn_actor_visuals_main_thread(
    query: Query<(Entity, &Actor, &Transform), Added<Actor>>,
    mut visuals: NonSendMut<VisualRegistry>,
) { ... }
```
   **Почему:** Сразу видно что система — main thread only, требует осторожности.

2. **Используй `NonSend<T>`/`NonSendMut<T>` для Godot resources:**
   - `NonSend<VisualRegistry>` — read-only доступ
   - `NonSendMut<VisualRegistry>` — mutable доступ
   - Bevy гарантирует main thread execution

3. **НЕ используй `Res<T>`/`ResMut<T>` для Gd<T> типов:**
   - ❌ `Res<VisualRegistry>` — compile error (Gd<Node3D> не Send+Sync)
   - ✅ `NonSend<VisualRegistry>` — работает, main thread only

4. **Минимизируй Godot calls в системах:**
   - Main thread = bottleneck
   - Тяжёлая ECS логика → отдельные системы (могут быть parallel)
   - Godot sync системы → только визуал updates (лёгкие операции)

**Как работает Change Detection:**
- Bevy отслеживает изменения компонентов через tick counter
- `Changed<T>` фильтрует только entity с `component.tick > system.last_run_tick`
- Автоматически (не нужно вручную отслеживать diffs)

**Частота sync:** Depends on system schedule (обычно каждый frame для визуалов).

**Performance considerations:**
- Main thread systems блокируют друг друга (sequential execution)
- `Changed<T>` критичен — обрабатываем только изменённые entity
- Если sync система тяжёлая → рефакторить в Command Pattern (ECS write events → SimulationBridge process)

### Пример полного цикла

#### 1. AI система принимает решение (ECS)

```rust
// === voidrun_simulation/src/ai/combat.rs ===

fn ai_combat_decision(
    mut query: Query<(Entity, &mut AIState, &Transform)>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    for (entity, mut state, transform) in query.iter_mut() {
        if let AIState::Idle = *state {
            if let Some((enemy, _)) = find_nearest(&enemies, transform.translation) {
                // Изменить state
                *state = AIState::Attacking;
                // ^^^ Bevy автоматически пометит AIState как Changed
            }
        }
    }
}
```

#### 2. Godot sync система видит изменение

```rust
// === voidrun_godot/src/animation_sync.rs ===

/// Sync AI state → AnimationPlayer
pub fn sync_ai_animations_main_thread(
    query: Query<(Entity, &AIState), Changed<AIState>>,
    visuals: NonSend<VisualRegistry>, // NonSend → main thread only
) {
    for (entity, state) in query.iter() {
        if let AIState::Attacking = state {
            if let Some(node) = visuals.visuals.get(&entity) {
                let mut anim_player = node.get_node_as::<AnimationPlayer>("AnimationPlayer");
                anim_player.play("attack_swing".into());
            }
        }
    }
}
```

#### 3. AnimationPlayer отправляет событие обратно (Rust через godot-rust)

```rust
// === voidrun_godot/src/nodes/animation_watcher.rs ===

use godot::prelude::*;
use voidrun_simulation::events::GodotAnimationEvent;

#[derive(GodotClass)]
#[class(base=Node)]
struct AnimationWatcher {
    base: Base<Node>,
    entity: Option<Entity>,
}

#[godot_api]
impl INode for AnimationWatcher {
    fn init(base: Base<Node>) -> Self {
        Self { base, entity: None }
    }

    fn ready(&mut self) {
        let player = self.base().get_node_as::<AnimationPlayer>("../AnimationPlayer");
        let callable = self.base().callable("on_anim_finished");
        player.connect("animation_finished".into(), callable);
    }
}

#[godot_api]
impl AnimationWatcher {
    #[func]
    fn on_anim_finished(&mut self, anim_name: GString) {
        if let Some(entity) = self.entity {
            // Получить World (через singleton или thread-safe queue)
            SIMULATION.lock().unwrap().send_event(GodotAnimationEvent::AnimationFinished {
                entity,
                animation: anim_name.to_string(),
            });
        }
    }
}
```

#### 4. ECS обрабатывает событие

```rust
// === voidrun_simulation/src/ai/systems.rs ===

fn process_animation_finished(
    mut events: EventReader<GodotAnimationEvent>,
    mut query: Query<&mut AIState>,
) {
    for event in events.read() {
        if let GodotAnimationEvent::AnimationFinished { entity, animation } = event {
            if animation == "attack_swing" {
                if let Ok(mut state) = query.get_mut(*entity) {
                    *state = AIState::Idle; // Вернуться в Idle после атаки
                }
            }
        }
    }
}
```

**Цикл замкнулся:** ECS → Changed → Godot animation → Event → ECS.

## Обоснование

### Почему НЕ GodotBridge trait

**Проблема:** Это **реинвент PresentationClient abstraction** (ADR-002).

```rust
// То что предлагалось:
pub trait GodotBridge {
    fn spawn_visual(&mut self, entity: Entity, prefab: &str, pos: Vec3);
    fn play_animation(&mut self, entity: Entity, anim: &str);
}

// То что было в presentation-layer-abstraction.md:
pub trait PresentationClient {
    fn spawn_visual(&mut self, entity: Entity, prefab: &str, pos: Vec3);
    fn play_animation(&mut self, entity: Entity, anim: &str);
}
```

**Это одно и то же!**

**ADR-002 решение:** POSTPONED (YAGNI) — Godot работает отлично, смена движка = риск <5%.

**Следствие:** Абстракция решает **несуществующую проблему**.

### Почему Bevy Events

**Преимущества:**

1. **Zero boilerplate**
   - `add_event::<T>()` — одна строка регистрации
   - `EventReader<T>` — встроенный механизм подписки
   - Не нужны custom queues, handlers, type erasure

2. **Модульность через типы**
   - Каждый модуль определяет свои Event типы
   - Bevy Event Bus = естественный pub/sub
   - Нет центрального god enum на тысячи строк

3. **Testability**
   ```rust
   #[test]
   fn test_damage_kills_entity() {
       let mut app = App::new();
       app.add_event::<GodotInputEvent>();

       let entity = app.world.spawn(Health { current: 50.0, max: 100.0 }).id();

       // Mock событие (без Godot)
       app.world.send_event(GodotInputEvent::WeaponHit {
           attacker: Entity::PLACEHOLDER,
           victim: entity,
       });

       app.update();
       // ... assertions
   }
   ```

4. **Понятность**
   - Прямолинейный data flow: Event → EventReader → logic
   - Нет абстракций (trait objects, downcasts)
   - Код читается как книга

**Trade-offs:**

- ❌ **Tight coupling** — Godot зависит от simulation types
- ✅ **Простота** — нет промежуточных слоёв
- ✅ **YAGNI** — решаем текущую задачу, не "на будущее"

### Почему Changed<T> для sync

**Bevy Change Detection** — встроенный механизм отслеживания изменений:

```rust
Query<&Health, Changed<Health>> // Только изменённые с прошлого запуска системы
Query<&Transform, Added<Transform>> // Только только что добавленные
```

**Преимущества:**

1. **Автоматическая фильтрация** — не надо вручную отслеживать diffs
2. **Performance** — Godot sync системы обрабатывают только изменённые entity
3. **Нет дубликатов** — каждое изменение обрабатывается ровно один раз

**Пример:** Если AIState изменился у 5 из 100 NPC, `Changed<AIState>` вернёт только 5.

## Влияния

### Новые файлы

**voidrun_simulation:**
- `src/events/combat.rs` — GodotCombatEvent (WeaponHit, Parry)
- `src/events/animation.rs` — GodotAnimationEvent (AnimationFinished, AnimationTrigger)
- `src/events/transform.rs` — GodotTransformEvent (ZoneTransition, PostSpawn, ArrivedAtDestination)
- `src/events/ai.rs` — GodotAIEvent (ActorSpotted, ActorLost)
- `src/events/input.rs` — PlayerInputEvent
- `src/combat/events.rs` — DamageDealt, EntityDied (domain events внутри ECS)
- `src/ai/events.rs` — ZoneTransitionEvent, AIGoalChanged (domain events внутри ECS)

**voidrun_godot:**
- `src/animation_sync.rs` — sync_ai_animations система
- `src/nodes/animation_watcher.rs` — AnimationWatcher (Rust GodotClass)
- `src/nodes/vision_cone.rs` — VisionCone (Rust GodotClass для AI perception)

### Изменения

**voidrun_simulation/src/lib.rs:**
```rust
pub mod events; // Export domain events

// App setup (ОБНОВЛЕНО: domain events вместо одного GodotInputEvent)
app.add_event::<events::GodotCombatEvent>()
   .add_event::<events::GodotAnimationEvent>()
   .add_event::<events::GodotTransformEvent>()
   .add_event::<events::GodotAIEvent>()
   .add_event::<events::PlayerInputEvent>()
   .add_event::<combat::DamageDealt>()
   .add_event::<ai::ZoneTransitionEvent>();
```

**voidrun_godot/src/simulation_bridge.rs:**
```rust
// Упрощается (не нужны Command/Event queues)
pub struct SimulationBridge {
    app: App, // Весь Bevy App
}

impl SimulationBridge {
    // Generic send_event для любого типа событий
    pub fn send_event<E: Event>(&mut self, event: E) {
        self.app.world.send_event(event);
    }

    pub fn tick(&mut self) {
        self.app.update(); // ECS + Godot sync системы
    }
}

// Использование:
// bridge.send_event(GodotCombatEvent::WeaponHit { ... });
// bridge.send_event(GodotAnimationEvent::AnimationFinished { ... });
```

**voidrun_godot/Cargo.toml:**
```toml
[dependencies]
voidrun_simulation = { path = "../voidrun_simulation" }
# ^^^ Direct dependency — OK!
```

### Удалённые компоненты

Из первоначального дизайна ADR-003:
- ~~`SimulationCommand` trait~~
- ~~`CommandHandler` trait~~
- ~~`CommandQueue` struct~~
- ~~Custom Event Bus~~

Всё заменено на Bevy Events.

### Тесты

**Unit tests (без Godot):**
```rust
#[test]
fn test_weapon_hit_deals_damage() {
    let mut app = App::new();
    app.add_event::<GodotCombatEvent>()
       .add_systems(Update, process_weapon_hits);

    let attacker = app.world.spawn(Weapon { damage: 25.0 }).id();
    let victim = app.world.spawn(Health { current: 100.0, max: 100.0 }).id();

    // Mock событие (ОБНОВЛЕНО: GodotCombatEvent)
    app.world.send_event(GodotCombatEvent::WeaponHit {
        attacker,
        victim,
        hitbox_name: "sword".into(),
    });

    app.update();

    let health = app.world.get::<Health>(victim).unwrap();
    assert_eq!(health.current, 75.0); // 100 - 25 = 75
}
```

**Integration tests (с Godot):**
- Godot scene → signal → send_event → ECS обработка
- Проверить что AnimationFinished корректно переключает AIState

## Альтернативы (отклонены)

### Trait-based GodotBridge

**Причина отклонения:** Реинвент PresentationClient (ADR-002 POSTPONED).

### Command/Event handlers с type erasure

```rust
trait CommandHandlerErased {
    fn handle(&mut self, cmd: &dyn Any, world: &mut World);
}
```

**Причина отклонения:** Громоздко, требует boilerplate, решает несуществующую проблему (modular handlers — уже есть через Event types).

### Macro-based dispatch

```rust
define_command_handlers! {
    CombatCommandHandler => CombatCommand,
    AICommandHandler => AICommand,
}
```

**Причина отклонения:** Over-engineering, Bevy Events проще и понятнее.

## Риски и митигация

### Риск 1: Tight coupling (Godot → Simulation)

**Описание:** Godot crate напрямую зависит от simulation types.

**Вероятность:** 100% (это дизайн решение)

**Влияние:** Низкое (Godot = единственный клиент, YAGNI)

**Митигация:**
- Если появится второй клиент (например Web viewer) → тогда создать PresentationClient abstraction
- До того момента — не создавать абстракцию "на всякий случай"

### Риск 2: Event flooding

**Описание:** Слишком много событий может замедлить ECS обработку.

**Вероятность:** Средняя (если отправлять события каждый frame для каждого entity)

**Влияние:** Среднее (FPS drops)

**Митигация:**
- Использовать Changed<T> queries (автоматическая фильтрация)
- Debounce события где возможно (например ZoneTransition — не чаще 1Hz)
- Профилирование (tracy, bevy_mod_debugdump)

**Метрики:**
- Event count per frame < 100 (нормально)
- Event count per frame > 1000 (проблема — надо оптимизировать)

### Риск 3: Забыть обработать событие

**Описание:** Событие отправлено, но нет системы с EventReader → событие потеряно.

**Вероятность:** Низкая (компилятор не поможет, но тесты покажут)

**Влияние:** Среднее (баг в геймплее)

**Митигация:**
- Unit tests для каждого критичного события
- Integration tests (Godot → Event → ECS → assertion)
- Logging (warn если событие не обработано за N frames)

## План имплементации

### Фаза 1: Event types (1 час)

1. Создать `voidrun_simulation/src/events.rs`
2. Определить `GodotInputEvent` enum (5-7 вариантов)
3. Регистрация в App: `app.add_event::<GodotInputEvent>()`

### Фаза 2: Godot event sender (1-2 часа)

4. `voidrun_godot/src/event_sender.rs` — helper для send_event
5. Singleton или thread-safe queue для доступа к World
6. Пример: AnimationWatcher → AnimationFinished event

### Фаза 3: ECS event handlers (2-3 часа)

7. `process_weapon_hits` система (GodotInputEvent::WeaponHit)
8. `process_animation_finished` система
9. `update_strategic_position` система (ZoneTransition)

### Фаза 4: Change Detection sync (2-3 часа)

10. `sync_ai_animations` (Changed<AIState>)
11. `sync_health_bars` (Changed<Health>)
12. `spawn_visuals` (Added<VisualPrefab>)

### Фаза 5: Tests (1-2 часа)

13. Unit tests для каждого event handler
14. Integration test: Godot signal → Event → ECS state change

**Итого:** 7-11 часов (~1-1.5 дня)

## Откат

Если подход не зайдёт (unlikely):

**План B: Command/Event queues**
- Вернуться к первоначальному дизайну из ADR-003
- `CommandQueue`, `EventQueue` структуры
- Больше boilerplate, но изолированнее

**План C: Trait-based GodotBridge**
- Создать PresentationClient abstraction
- Godot реализует trait
- Максимум гибкости, максимум сложности

**Критерии для отката:**
- Event flooding (>1000 events/frame регулярно)
- Непонятный data flow (код сложно читать)
- Появление второго клиента (Web viewer, headless server)

**Вероятность отката:** <10%

## Заключение

**Bevy Events + Change Detection** = простое, понятное, модульное решение для Command/Event архитектуры.

**Ключевые принципы:**
- YAGNI — не создаём абстракции "на будущее"
- KISS — используем встроенные механизмы Bevy
- Модульность — через типы (не через traits)
- Testability — mock events без Godot
- **Domain Events** — разделение по доменам (combat, AI, transform, animation)

**Обновления (2025-01-10):**
1. **Domain Events** — вместо одного `GodotInputEvent` используем `GodotCombatEvent`, `GodotAnimationEvent`, `GodotTransformEvent`, `GodotAIEvent`
2. **Rust-only** — все Godot nodes (AnimationWatcher, VisionCone) пишутся на Rust через godot-rust (никакого GDScript)
3. **PostSpawn event** — Godot отправляет точную позицию после spawn → детерминистичные saves

**Следующие шаги:** См. План имплементации (Фаза 1-5).

---

**См. также:**
- [ADR-002: Godot-Rust Integration Pattern](ADR-002-godot-rust-integration-pattern.md) — YAGNI для Presentation abstraction
- [ADR-003: ECS vs Godot Physics Ownership](ADR-003-ecs-vs-godot-physics-ownership.md) — Hybrid Architecture
- [ADR-005: Transform Ownership & Strategic Positioning](ADR-005-transform-ownership-strategic-positioning.md) — StrategicPosition component
