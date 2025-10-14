# Bevy ECS Reference Guide

**Цель:** Comprehensive reference по внутренностям Bevy ECS и best practices для VOIDRUN проекта.

**Аудитория:** Разработчики, которым нужно понять "как оно работает под капотом" и как писать эффективный ECS код.

---

## Table of Contents

1. [SystemParam: Магия Dependency Injection](#1-systemparam-магия-dependency-injection)
2. [Query Filters: With, Without, Changed, Added](#2-query-filters-with-without-changed-added)
3. [Archetype-Based Storage](#3-archetype-based-storage)
4. [Commands: Deferred Operations](#4-commands-deferred-operations)
5. [Events System](#5-events-system)
6. [Change Detection](#6-change-detection)
7. [System Scheduling & Parallelism](#7-system-scheduling--parallelism)
8. [Custom SystemParam](#8-custom-systemparam)
9. [Performance Best Practices](#9-performance-best-practices)
10. [Quick Reference](#10-quick-reference)

---

## 1. SystemParam: Магия Dependency Injection

### Как Bevy понимает, какие параметры передать в систему?

**Compile-time анализ через Rust type system:**

```rust
fn my_system(
    query: Query<&Health>,
    time: Res<Time>,
    mut commands: Commands,
) {
    // Bevy автоматически инжектит параметры
}
```

### Под капотом: Трейт SystemParam

**Каждый параметр реализует `SystemParam` трейт:**

```rust
pub trait SystemParam {
    type State: Send + Sync + 'static;  // Persistent state между вызовами
    type Item<'w, 's>: SystemParam;     // Actual param с lifetimes

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State;
    fn get_param<'w, 's>(state: &'s mut Self::State, world: UnsafeWorldCell<'w>) -> Self::Item<'w, 's>;
}
```

**Lifetimes:**
- `'w` (world) — данные из World (components, resources)
- `'s` (state) — state параметра (например, cursor для EventReader)

### Процесс инъекции

1. **Регистрация системы:**
   ```rust
   app.add_systems(Update, my_system);
   ```

2. **IntoSystem trait конвертирует функцию:**
   ```rust
   // Упрощенно:
   impl<Param: SystemParam> IntoSystem for fn(Param) {
       type System = FunctionSystem<fn(Param)>;
   }
   ```

3. **init_state вызывается один раз:**
   ```rust
   // Для Query<&Health>:
   QueryState::new(world) // Анализирует archetype graph, строит access plan
   ```

4. **get_param вызывается каждый фрейм:**
   ```rust
   // Возвращает Query с доступом к нужным archetypes
   Query::new(world, state)
   ```

### Data Access Анализ

**Bevy анализирует read/write паттерны:**

```rust
fn system_a(query: Query<&Health>) {}       // Read Health
fn system_b(query: Query<&mut Health>) {}   // Write Health

// system_a + system_b = CONFLICT → не могут запускаться параллельно
```

**Scheduler использует этот анализ для автоматического параллелизма!**

---

## 2. Query Filters: With, Without, Changed, Added

### Базовая структура Query

```rust
Query<D, F>
// D = Data (что достаем из entity)
// F = Filter (какие entity подходят, опционально)
```

### With\<T\> / Without\<T\>

**Фильтрация по наличию компонента БЕЗ доставания данных:**

```rust
// Достаем Health, но ТОЛЬКО у entity с Player (Player НЕ достается)
Query<&Health, With<Player>>

// Entity должна иметь Health И Player, но НЕ Enemy
Query<&Health, (With<Player>, Without<Enemy>)>
```

**Почему это важно:**

1. **Performance**: `With<T>` не загружает данные компонента, только проверяет archetype
2. **Избежать query конфликтов:**

```rust
fn update_health(
    // Оба query могут брать &mut Health БЕЗ конфликта!
    mut players: Query<&mut Health, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<&mut Health, (With<Enemy>, Without<Player>)>,
) {
    // Entity с Player+Enemy одновременно будут пропущены обоими query
}
```

### Added\<T\> / Changed\<T\>

**Change detection фильтры:**

```rust
// Только entity, где Health был добавлен с последнего запуска системы
Query<&Health, Added<Health>>

// Только entity, где Health был изменен (DerefMut)
Query<&Health, Changed<Health>>
```

**Как работает:**

- Bevy отслеживает **tick** (timestamp) для каждого компонента
- Система запоминает **last_run_tick**
- При следующем запуске: `if component_tick > last_run_tick` → фильтр проходит

**Важное замечание:**

```rust
fn my_system(mut query: Query<&mut Health>) {
    for mut health in &mut query {
        // ❌ Просто доступ &mut НЕ триггерит Changed
        let _ = health.0;

        // ✅ Только DerefMut (реальное изменение) триггерит
        health.0 = 100;  // Changed<Health> = true
    }
}
```

### Комбинирование фильтров

```rust
// Tuple = AND (все условия должны выполняться)
Query<&Health, (With<Player>, Without<Enemy>, Changed<Health>)>

// Or<> = OR (хотя бы одно условие)
Query<&Health, Or<(With<Player>, With<Enemy>)>>

// Сложные комбинации
Query<&Health, (
    Or<(Added<Health>, Changed<Health>)>,  // Новые ИЛИ изменённые
    Without<Dead>,                         // Но НЕ мертвые
)>
```

### VOIDRUN Примеры

```rust
// Только живые акторы с AI
Query<(&Actor, &AIState), (With<Health>, Without<Dead>)>

// Оружие, которое только что выстрелило
Query<&Weapon, Changed<Weapon>>

// Новые повреждения для визуализации
Query<(&Health, &Actor), Changed<Health>>
```

---

## 3. Archetype-Based Storage

### Что такое Archetype?

**Archetype = уникальная комбинация компонентов:**

```rust
Archetype A: [Position, Velocity, Health]
Archetype B: [Position, Velocity, Health, Player]
Archetype C: [Position, Health, Enemy]
```

**Ключевой принцип:** World имеет ОДИН archetype для каждой уникальной комбинации.

### Struct-of-Arrays (SoA) Layout

**Каждый archetype хранит компоненты в отдельных `Vec<T>`:**

```
Archetype [Position, Velocity, Health] (100 entities):
┌─────────────────────────────────┐
│ Position[] → [pos1, pos2, ..., pos100]  ← Contiguous array
│ Velocity[] → [vel1, vel2, ..., vel100]  ← Contiguous array
│ Health[]   → [hp1,  hp2,  ..., hp100]   ← Contiguous array
└─────────────────────────────────┘
```

**Преимущества:**

1. **Cache locality**: Все `Position` лежат рядом в памяти → CPU cache friendly
2. **SIMD-friendly**: Можно обрабатывать векторно (4-8 элементов за раз)
3. **Query optimization**: Bevy итерируется по всему archetype без пропусков

### Archetype Transitions

**Entity перемещается между archetypes при изменении компонентов:**

```rust
// Spawn entity
let entity = commands.spawn((Position::default(), Velocity::default()));
// → Archetype [Position, Velocity]

// Insert component
commands.entity(entity).insert(Health(100));
// → MOVE to Archetype [Position, Velocity, Health]
// ⚠️ ДОРОГАЯ ОПЕРАЦИЯ: копирование данных между tables!

// Remove component
commands.entity(entity).remove::<Velocity>();
// → MOVE to Archetype [Position, Health]
```

**Performance Implications:**

- **Archetype transition = expensive** (копирование всех компонентов entity)
- **Частые add/remove** = fragmentation + performance hit

### Edges Cache

**Bevy кеширует переходы между archetypes:**

```rust
// Первый раз: compute new archetype
commands.entity(e1).insert(Health(100));  // Slow

// Второй раз: lookup в cache
commands.entity(e2).insert(Health(100));  // Fast (cached edge)
```

**Edge = "путь" из Archetype A в Archetype B при добавлении/удалении компонента.**

### Best Practices

**❌ ПЛОХО (частые transitions):**

```rust
// Каждый фрейм:
if condition {
    commands.entity(e).insert(Stunned);
} else {
    commands.entity(e).remove::<Stunned>();
}
```

**✅ ХОРОШО (marker component остается):**

```rust
#[derive(Component)]
struct Stunned {
    active: bool,
}

// Один раз при spawn:
commands.spawn((Actor::default(), Stunned { active: false }));

// Каждый фрейм (без archetype transition):
stunned.active = condition;
```

### VOIDRUN Application

**В нашем проекте:**

- `Actor`, `Health`, `Weapon` — всегда вместе (один archetype)
- `AIState` — отдельно (только у NPC)
- `Player` — marker component (без данных)

```rust
// Типичные archetypes VOIDRUN:
[Actor, Health, Weapon, AIState, StrategicPosition]  // NPC
[Actor, Health, Weapon, Player, StrategicPosition]   // Player
```

---

## 4. Commands: Deferred Operations

### Почему Commands не применяются сразу?

**Thread Safety + Параллелизм:**

```rust
fn system_a(mut commands: Commands) {
    commands.spawn(PlayerBundle::default());  // Записывается в queue
}

fn system_b(query: Query<&Player>) {
    // system_b может читать World параллельно с system_a!
    // Потому что Commands только записывает в Vec (не изменяет World)
}
```

**Ключевая идея:**

- `Commands` = **thread-safe queue** операций (только `&World` нужен)
- Операции **применяются позже** когда scheduler может взять `&mut World`
- Это позволяет **параллельно запускать системы** без data races

### Основные операции

```rust
// Spawn (получаем Entity ID сразу, но entity создается позже)
let entity = commands.spawn((
    Position::default(),
    Velocity::default(),
)).id();  // ← Entity ID reserved

// Insert/Remove components
commands.entity(entity)
    .insert(Health(100))
    .remove::<Velocity>();

// Despawn
commands.entity(entity).despawn();

// Spawn batch (эффективнее чем много spawn по отдельности)
commands.spawn_batch(vec![
    (Position::default(), Velocity::default()),
    (Position::default(), Velocity::default()),
]);
```

### Когда Commands применяются?

**В пределах одного Schedule:**

```
Update Schedule:
  ┌─ system_a (Commands queue writes)
  │
  ├─ system_b (Commands queue writes)
  │
  ├─ apply_deferred()  ← Commands применяются к World
  │
  └─ system_c (видит изменения от system_a и system_b)
```

**Между Schedules:**

```
Update Schedule:
  systems...
  → apply_deferred()

PostUpdate Schedule:
  systems... (видят все Commands из Update)
```

**Явный порядок:**

```rust
app.add_systems(Update, (
    spawn_enemies,
    process_enemies.after(spawn_enemies),  // ❌ НЕ увидит новых врагов!
));

// Решение: apply_deferred между системами
app.add_systems(Update, (
    spawn_enemies,
    apply_deferred,  // ← Принудительно применить Commands
    process_enemies,
));
```

### VOIDRUN Usage

```rust
// SimulationBridge: ECS → Godot commands
commands.entity(entity).insert(AttachPrefabCommand {
    prefab_path: "res://actors/player.tscn".into(),
});

// Combat: spawn projectile
commands.spawn((
    Projectile { damage: 50.0 },
    StrategicPosition { chunk, offset },
));
```

---

## 5. Events System

### Как работают Events?

**Ring buffer с автоматической очисткой:**

```rust
#[derive(Event)]
struct DamageDealt {
    target: Entity,
    amount: f32,
}

// Регистрация
app.add_event::<DamageDealt>();
```

**Внутри Events хранит два буфера:**

```
Frame N:   buffer_a: [event1, event2]
           buffer_b: []

Frame N+1: buffer_a: [event1, event2]        ← Старые события ЕЩЁ живы
           buffer_b: [event3, event4]        ← Новые события

Frame N+2: buffer_a: [event5]                ← event1, event2 удалены
           buffer_b: [event3, event4]
```

**Lifecycle:** События живут **минимум 2 фрейма** (или 2 fixed timestep цикла).

### EventWriter / EventReader

```rust
// Отправка событий
fn deal_damage(
    mut ev_damage: EventWriter<DamageDealt>,
    query: Query<&Weapon>,
) {
    ev_damage.send(DamageDealt {
        target: enemy_entity,
        amount: 50.0,
    });
}

// Чтение событий
fn apply_damage(
    mut ev_damage: EventReader<DamageDealt>,
    mut query: Query<&mut Health>,
) {
    for ev in ev_damage.read() {
        if let Ok(mut health) = query.get_mut(ev.target) {
            health.0 -= ev.amount;
        }
    }
}
```

**EventReader state:**

- Каждый `EventReader` хранит **internal cursor** (последний прочитанный event)
- Разные системы **независимо** читают одни и те же события
- `ev_damage.read()` возвращает **только новые** события с последнего вызова

### Важные моменты

**⚠️ События могут быть пропущены:**

```rust
// Если система не запускается каждый фрейм:
fn handle_damage(
    mut ev_damage: EventReader<DamageDealt>,
) {
    // Если система пропустит 2+ фрейма → старые события удалятся!
}

app.add_systems(Update, handle_damage.run_if(some_condition));  // ❌ Опасно!
```

**⚠️ Может быть 1-frame lag:**

```rust
// Frame N:
fn system_a(mut ev: EventWriter<Foo>) {
    ev.send(Foo);  // Отправили событие
}

fn system_b(mut ev: EventReader<Foo>) {
    // Если system_b запустилась РАНЬШЕ system_a → не увидит событие в этом фрейме
}

// Решение: явный порядок
app.add_systems(Update, (system_a, system_b.after(system_a)));
```

### VOIDRUN Events

```rust
// Combat events
#[derive(Event)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: f32,
}

#[derive(Event)]
pub struct EntityDied {
    pub entity: Entity,
}

// AI events
#[derive(Event)]
pub struct ActorSpotted {
    pub spotter: Entity,
    pub target: Entity,
}
```

**Паттерн:** ECS systems отправляют события → Godot systems читают для визуализации.

---

## 6. Change Detection

### Как Bevy отслеживает изменения?

**Tick-based система:**

```rust
// Внутри Bevy (упрощенно):
struct ComponentTicks {
    added: Tick,    // Когда component добавлен к entity
    changed: Tick,  // Когда component последний раз изменен
}

// Каждая система запоминает:
struct SystemMeta {
    last_run: Tick,  // Последний запуск системы
}
```

**Query фильтр проверяет:**

```rust
// Changed<Health> = (health.changed > system.last_run)
Query<&Health, Changed<Health>>

// Added<Health> = (health.added > system.last_run)
Query<&Health, Added<Health>>
```

### Mut\<T\> и триггеры

**`Mut<T>` = smart pointer с change tracking:**

```rust
fn my_system(mut query: Query<&mut Health>) {
    for mut health in &mut query {
        // ✅ Триггерит Changed<Health>:
        *health = Health(100);           // DerefMut
        health.0 = 100;                  // DerefMut

        // ✅ Умное изменение (только если != текущему):
        health.set_if_neq(Health(100));  // Только если health != 100

        // ❌ НЕ триггерит Changed:
        let value = health.0;            // Просто чтение
        let _ = &*health;                // Deref (не Mut)
    }
}
```

**Ключевая идея:** Change detection срабатывает на **`DerefMut`**, не на сравнение значений.

### Bypass Change Detection

**Иногда нужно изменить без триггера:**

```rust
fn my_system(mut query: Query<&mut Health>) {
    for mut health in &mut query {
        // Bypass change detection
        health.bypass_change_detection().0 = 100;

        // Используй для internal bookkeeping, не для логики игры!
    }
}
```

### Best Practice: set_if_neq

**Избегай ложных Changed триггеров:**

```rust
// ❌ ПЛОХО: Триггерит Changed даже если значение не изменилось
fn update_position(mut query: Query<&mut Position>) {
    for mut pos in &mut query {
        *pos = calculate_position();  // Может быть == старому значению
    }
}

// ✅ ХОРОШО: Changed только если реально изменилось
fn update_position(mut query: Query<&mut Position>) {
    for mut pos in &mut query {
        let new_pos = calculate_position();
        pos.set_if_neq(new_pos);  // Требует PartialEq
    }
}
```

### VOIDRUN Application

```rust
// Sync ECS → Godot (только изменённые)
fn sync_visual_system(
    query: Query<(&Actor, &Health), Changed<Health>>,
    // Changed<Health> = только entity с изменённым HP
) {
    for (actor, health) in &query {
        // Update health bar в Godot
    }
}
```

---

## 7. System Scheduling & Parallelism

### Автоматический параллелизм

**Bevy анализирует data access patterns:**

```rust
fn system_a(q: Query<&Position>) {}           // Read Position
fn system_b(q: Query<&Velocity>) {}           // Read Velocity
fn system_c(q: Query<(&Position, &Velocity)>) {}  // Read both

// Все три системы могут запускаться ПАРАЛЛЕЛЬНО!
// Нет конфликтов (только чтение)
```

**Конфликты:**

```rust
fn system_d(q: Query<&mut Position>) {}  // Write Position
fn system_e(q: Query<&Position>) {}      // Read Position

// system_d и system_e НЕ МОГУТ запускаться параллельно
// Exclusive access (Write) конфликтует с Shared (Read)
```

### System Conflicts

**Типы конфликтов:**

1. **Component access:**
   - `Query<&T>` + `Query<&T>` → ✅ OK (shared read)
   - `Query<&mut T>` + `Query<&T>` → ❌ Conflict
   - `Query<&mut T>` + `Query<&mut T>` → ❌ Conflict

2. **Resource access:**
   - `Res<T>` + `Res<T>` → ✅ OK
   - `ResMut<T>` + `Res<T>` → ❌ Conflict
   - `ResMut<T>` + `ResMut<T>` → ❌ Conflict

3. **Disjoint queries (с фильтрами):**
   ```rust
   Query<&mut Health, With<Player>>    // ✅ OK
   Query<&mut Health, With<Enemy>>     // ✅ OK (disjoint sets)
   ```

### Explicit Ordering

```rust
// Порядок выполнения
app.add_systems(Update, (
    player_input,
    apply_velocity.after(player_input),
    check_collisions.after(apply_velocity),
));

// Chaining (последовательно, один за другим)
app.add_systems(Update, (
    player_input,
    apply_velocity,
    check_collisions,
).chain());
```

**Разница:**

- `.after()` / `.before()` — зависимость, но системы могут запуститься с разрывом
- `.chain()` — строгая последовательность, без других систем между ними

### SystemSets

**Группировка систем:**

```rust
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum PhysicsSet {
    Movement,
    Collision,
    PostPhysics,
}

app.configure_sets(Update, (
    PhysicsSet::Movement,
    PhysicsSet::Collision.after(PhysicsSet::Movement),
    PhysicsSet::PostPhysics.after(PhysicsSet::Collision),
));

app.add_systems(Update, (
    apply_velocity.in_set(PhysicsSet::Movement),
    check_collisions.in_set(PhysicsSet::Collision),
));
```

**Преимущества:**

- Логическая группировка
- Легко добавлять новые системы в существующие sets
- Контроль порядка на уровне групп

### VOIDRUN Scheduling

```rust
// SimulationPlugin:
app.add_systems(FixedUpdate, (
    // Combat systems (могут быть параллельны)
    weapon_system,
    damage_system,
    health_system,
).chain());  // Но мы форсим последовательность для предсказуемости

// Godot sync systems (параллельны, только визуалы)
app.add_systems(PostUpdate, (
    visual_sync_system,    // Changed<Health> → Godot
    movement_sync_system,  // Changed<Position> → Godot
    weapon_sync_system,    // WeaponFired event → Godot
));
```

---

## 8. Custom SystemParam

### Зачем нужны Custom SystemParam?

**Инкапсуляция сложной логики:**

```rust
// Вместо этого в каждой системе:
fn my_system(
    query1: Query<&Player>,
    query2: Query<&Enemy>,
    res1: Res<GameConfig>,
    res2: ResMut<Score>,
) { }

// Можно сделать:
fn my_system(game_state: MyGameState) { }
```

### Создание Custom SystemParam

**Базовый пример:**

```rust
#[derive(SystemParam)]
struct PlayerCounter<'w, 's> {
    players: Query<'w, 's, &'static Player>,
    count: ResMut<'w, PlayerCount>,
}

impl<'w, 's> PlayerCounter<'w, 's> {
    fn update_count(&mut self) {
        self.count.0 = self.players.iter().len();
    }
}

// Использование:
fn count_players(mut counter: PlayerCounter) {
    counter.update_count();
}
```

**Правила:**

1. `#[derive(SystemParam)]` макрос
2. Lifetimes: `'w` (world), `'s` (state)
3. Все поля должны реализовывать `SystemParam`

### Advanced: Custom State

**Если нужен кастомный state:**

```rust
pub struct MyParamState {
    cached_data: Vec<Entity>,
}

pub struct MyParam<'w> {
    entities: &'w Vec<Entity>,
}

impl SystemParam for MyParam<'_> {
    type State = MyParamState;
    type Item<'w, 's> = MyParam<'w>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        // Инициализация state (один раз)
        MyParamState {
            cached_data: Vec::new(),
        }
    }

    fn get_param<'w, 's>(
        state: &'s mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        // Возвращаем param (каждый фрейм)
        MyParam {
            entities: &state.cached_data,
        }
    }
}
```

### VOIDRUN Example

```rust
// Полезный SystemParam для combat logic:
#[derive(SystemParam)]
pub struct CombatContext<'w, 's> {
    weapons: Query<'w, 's, &'static Weapon>,
    health: Query<'w, 's, &'static mut Health>,
    actors: Query<'w, 's, &'static Actor>,
    damage_events: EventWriter<'w, DamageDealt>,
}

impl<'w, 's> CombatContext<'w, 's> {
    pub fn deal_damage(&mut self, attacker: Entity, target: Entity) {
        let weapon = self.weapons.get(attacker).unwrap();
        let mut health = self.health.get_mut(target).unwrap();

        health.0 -= weapon.damage;
        self.damage_events.send(DamageDealt {
            attacker,
            target,
            damage: weapon.damage,
        });
    }
}

// Системы становятся чище:
fn melee_attack_system(mut combat: CombatContext) {
    combat.deal_damage(attacker, target);
}
```

---

## 9. Performance Best Practices

### 1. Избегай частых Archetype Transitions

**❌ ПЛОХО:**

```rust
// Каждый фрейм add/remove:
if stunned {
    commands.entity(e).insert(Stunned);
} else {
    commands.entity(e).remove::<Stunned>();
}
```

**✅ ХОРОШО:**

```rust
// Component с bool флагом (остается в archetype):
#[derive(Component)]
struct Stunned { active: bool }

stunned.active = is_stunned;  // Без archetype transition
```

### 2. Используй With<>/Without<> где возможно

**❌ ПЛОХО:**

```rust
Query<(&Position, Option<&Player>)>
// Достает Player данные (даже если не нужны)
```

**✅ ХОРОШО:**

```rust
Query<&Position, With<Player>>
// Только проверяет archetype, не загружает данные
```

### 3. Changed<T> фильтры для реактивности

**❌ ПЛОХО (обрабатываем всё каждый фрейм):**

```rust
fn sync_health(query: Query<(&Actor, &Health)>) {
    for (actor, health) in &query {
        // Update Godot health bar (каждый фрейм!)
    }
}
```

**✅ ХОРОШО (только изменённые):**

```rust
fn sync_health(query: Query<(&Actor, &Health), Changed<Health>>) {
    // Только entity с изменённым Health
}
```

### 4. Batch Commands операции

**❌ ПЛОХО:**

```rust
for i in 0..1000 {
    commands.spawn(EnemyBundle::default());
}
```

**✅ ХОРОШО:**

```rust
let enemies: Vec<_> = (0..1000)
    .map(|_| EnemyBundle::default())
    .collect();
commands.spawn_batch(enemies);  // Эффективнее
```

### 5. set_if_neq() для избежания ложных Changed

**❌ ПЛОХО:**

```rust
*position = new_position;  // Всегда триггерит Changed
```

**✅ ХОРОШО:**

```rust
position.set_if_neq(new_position);  // Changed только если != old
```

### 6. ParallelCommands для multi-threaded spawn

**Если spawn в параллельной системе:**

```rust
use bevy::ecs::system::ParallelCommands;

fn parallel_spawn(par_commands: ParallelCommands) {
    (0..1000).into_par_iter().for_each(|i| {
        par_commands.command_scope(|mut commands| {
            commands.spawn(EnemyBundle::default());
        });
    });
}
```

### 7. Избегай излишних Query

**❌ ПЛОХО:**

```rust
fn my_system(
    query1: Query<&Position>,
    query2: Query<&Velocity>,
) {
    // Две отдельные итерации
}
```

**✅ ХОРОШО:**

```rust
fn my_system(
    query: Query<(&Position, &Velocity)>,
) {
    // Одна итерация
}
```

### 8. Local<T> для per-system state

**Вместо static или global state:**

```rust
fn my_system(mut counter: Local<u32>) {
    *counter += 1;
    println!("System ran {} times", *counter);
}
```

---

## 10. Quick Reference

### SystemParam Types (Built-in)

| Type | Access | Description |
|------|--------|-------------|
| `Query<D, F>` | Components | Доступ к entity components с фильтрами |
| `Res<T>` | Resource | Read-only ресурс |
| `ResMut<T>` | Resource | Mutable ресурс |
| `Commands` | World | Deferred spawn/despawn/insert/remove |
| `EventReader<T>` | Events | Чтение событий |
| `EventWriter<T>` | Events | Отправка событий |
| `Local<T>` | System-local | Per-system локальный state |
| `Option<Res<T>>` | Resource | Optional ресурс (может не существовать) |
| `&World` | World | Read-only весь World (редко нужен) |
| `&mut World` | World | Exclusive World access (блокирует параллелизм!) |

### Query Filters

| Filter | Description |
|--------|-------------|
| `With<T>` | Entity должна иметь компонент T (не достается) |
| `Without<T>` | Entity НЕ должна иметь компонент T |
| `Changed<T>` | T был изменен с последнего запуска системы |
| `Added<T>` | T был добавлен к entity с последнего запуска |
| `Or<(A, B)>` | Хотя бы одно условие (A ИЛИ B) |
| `(A, B, C)` | Все условия (A И B И C) |

### Archetype Transition Triggers

| Operation | Transition? |
|-----------|-------------|
| `commands.spawn(...)` | ✅ Создает entity в archetype |
| `commands.entity(e).insert(T)` | ✅ Перемещает в новый archetype |
| `commands.entity(e).remove<T>()` | ✅ Перемещает в новый archetype |
| `query.get_mut(e)` | ❌ Нет (остается в archetype) |
| `*component = value` | ❌ Нет (изменение данных) |

### Change Detection Triggers

| Operation | Triggers Changed? |
|-----------|-------------------|
| `*component = value` | ✅ Да (DerefMut) |
| `component.0 = value` | ✅ Да (DerefMut через field) |
| `component.set_if_neq(value)` | ✅ Только если `!=` old |
| `let x = component.0` | ❌ Нет (read) |
| `component.bypass_change_detection()` | ❌ Нет (явный bypass) |

### System Ordering

```rust
// Параллельно (default)
app.add_systems(Update, (system_a, system_b));

// Последовательно
app.add_systems(Update, (system_a, system_b).chain());

// Зависимости
app.add_systems(Update, (
    system_a,
    system_b.after(system_a),
));

// В SystemSet
app.add_systems(Update, system_a.in_set(MySet));
```

### Events Lifecycle

```
Frame N:   Send event1, event2
Frame N+1: event1, event2 ЖИВЫ + send event3
Frame N+2: event1, event2 УДАЛЕНЫ, event3 ЖИВ
```

**Минимальная жизнь:** 2 frame updates или 2 fixed timestep cycles.

### Commands Apply Points

```
Schedule:
  system_a (Commands queued)
  system_b (Commands queued)
  → apply_deferred()  ← Applied here
  system_c (sees changes)

Between schedules:
  Update → apply_deferred() → PostUpdate
```

---

## VOIDRUN-Specific Patterns

### ECS Strategic Layer

```rust
// Только game state, без визуалов
#[derive(Component)]
pub struct Actor {
    pub name: String,
}

#[derive(Component)]
pub struct Health(pub f32);

// Bevy Events для domain logic
#[derive(Event)]
pub struct DamageDealt {
    pub target: Entity,
    pub damage: f32,
}
```

### Godot Sync (Tactical Layer)

```rust
// Changed<T> для efficient sync
fn sync_visual(
    query: Query<(&Actor, &Health), Changed<Health>>,
    mut godot_nodes: Query<&mut GodotNode>,
) {
    // Update только изменённые entity
}

// Commands для attach prefabs
commands.entity(entity).insert(AttachPrefabCommand {
    prefab_path: "res://actors/player.tscn".into(),
});
```

### Hybrid Position System

```rust
// ECS = strategic (chunk-based)
#[derive(Component)]
pub struct StrategicPosition {
    pub chunk: ChunkCoord,
    pub offset: Vec3,
}

// Godot = tactical (physics Transform)
// Transform authoritative для physics, sync обратно в ECS 0.1-1 Hz
```

---

## Further Reading

**Official Docs:**
- [Unofficial Bevy Cheat Book](https://bevy-cheatbook.github.io/) — лучший практический reference
- [Bevy Examples](https://github.com/bevyengine/bevy/tree/main/examples/ecs) — официальные примеры
- [docs.rs/bevy](https://docs.rs/bevy/latest/bevy/ecs/) — API документация

**VOIDRUN Docs:**
- [docs/architecture/bevy-ecs-design.md](../architecture/bevy-ecs-design.md) — Архитектурные решения ECS в проекте
- [docs/architecture/physics-architecture.md](../architecture/physics-architecture.md) — Hybrid ECS/Godot design
- [ADR-004: Command/Event Architecture](../decisions/ADR-004-command-event-architecture.md)

---

**Версия:** 1.0
**Дата:** 2025-01-15
**Автор:** Generated from research session
