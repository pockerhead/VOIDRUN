# Schedule & Melee Combat Refactoring Plan

**Date:** 2025-01-17
**Status:** APPROVED
**Priority:** HIGH (детерминизм + player combat support)

---

## Контекст и мотивация

### Проблема 1: Frame-dependent Schedules (недетерминизм)

**Текущая реализация:** `SlowUpdate` schedule использует `delta` из Godot `process()`:
```rust
// ❌ ПРОБЛЕМА: зависит от FPS
timer.timer += delta as f32;  // delta из process() - frame-dependent!
if timer.timer >= 0.3 {
    world.run_schedule(SlowUpdate);
}
```

**Почему плохо:**
- FPS падает → delta увеличивается → schedule срабатывает чаще
- FPS растёт → delta уменьшается → schedule срабатывает реже
- **Недетерминистичное поведение AI!**

### Проблема 2: Target-based Melee Combat (не работает для player)

**Текущая реализация:**
```rust
pub struct MeleeAttackIntent {
    pub target: Entity,  // ← AI знает заранее, Player НЕ знает!
}
```

**Почему плохо:**
- Player не знает target до hitbox collision
- Telegraph система не работает для player атак
- AI не может парировать player атаки

---

## Архитектурные решения

### Решение 1: Fixed Tick-based Schedules (детерминизм)

**Архитектура:**
```
FixedUpdate (60 Hz) - source of truth
  ├─ increment_tick_counter (каждый tick)
  ├─ run_slow_update_timer (каждый 20-й tick → SlowUpdate)
  └─ run_combat_update_timer (каждый 6-й tick → CombatUpdate)

SlowUpdate schedule (3 Hz = 60/20)
  ├─ poll_vision_cones_main_thread
  └─ update_combat_targets_main_thread

CombatUpdate schedule (10 Hz = 60/6)
  └─ detect_melee_windups_main_thread
```

**Частоты:**
- **FixedUpdate:** 60 Hz (каждые 0.0167s) - изменить с 64 Hz
- **SlowUpdate:** 3 Hz (каждые 0.333s = 20 ticks)
- **CombatUpdate:** 10 Hz (каждые 0.1s = 6 ticks)

**Преимущества:**
- ✅ Детерминистично (не зависит от FPS)
- ✅ Точные интервалы (tick counter не дрейфует)
- ✅ Wraparound safe (modulo handle u64 overflow)
- ✅ Легко добавлять новые частоты

### Решение 2: Visual Windup Detection (реалистичный AI)

**Event flow:**
```
1. Attacker starts windup (MeleeAttackState added)
   ↓
2. detect_melee_windups_main_thread (CombatUpdate, 10 Hz)
   - Spatial query: enemies within weapon range
   - Angle check: attacker facing defender (60° cone)
   - Visibility: defender in attacker's SpottedEnemies
   - Emit: GodotAIEvent::EnemyWindupVisible
   ↓
3. AI combat decision system
   - Defender decides: parry or continue attacking
   - Emit ParryIntent if chosen
```

**Hardcoded параметры (балансинг позже):**
- `angle_threshold`: 60° cone (dot product > 0.5)
- `detection_frequency`: 10 Hz (CombatUpdate)

**Преимущества:**
- ✅ Универсально для player И AI (симуляция играет сама в себя)
- ✅ Реалистично (AI видит замах визуально)
- ✅ Multi-target support (все в радиусе получают telegraph)

---

## Детальный план действий

### ФАЗА 1: Завершить Player Input ⏳

**Статус:** IN PROGRESS (осталось исправить ошибки компиляции)

**Задачи:**
1. ✅ Создать Player component
2. ✅ Создать input module (events, systems, controller)
3. ✅ Добавить Without<Player> фильтр в AI системы
4. ⏳ Исправить ошибки компиляции:
   - `glam::Vec2` → `Vec2` (уже импортирован)
   - `get_single()` → `single()` (deprecated)
   - `send()` → `write()` (deprecated)
5. 🔲 Протестировать spawn player + movement

**Файлы:**
- `crates/voidrun_godot/src/input/controller.rs`
- `crates/voidrun_godot/src/input/systems.rs`

---

### ФАЗА 2: Schedule Refactoring (детерминизм) 🎯

**Приоритет:** ВЫСОКИЙ (критично для детерминизма)

#### Шаг 2.1: Изменить FixedUpdate частоту (64 Hz → 60 Hz)

**Файл:** `crates/voidrun_simulation/src/lib.rs`

```rust
// ❌ БЫЛО:
.insert_resource(Time::<Fixed>::from_hz(64.0))

// ✅ СТАЛО:
.insert_resource(Time::<Fixed>::from_hz(60.0))  // Легче считать интервалы
```

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs`

```rust
// ❌ БЫЛО:
.insert_resource(Time::<Fixed>::from_hz(64.0))

// ✅ СТАЛО:
.insert_resource(Time::<Fixed>::from_hz(60.0))
```

#### Шаг 2.2: Создать FixedTickCounter resource

**Файл:** `crates/voidrun_simulation/src/lib.rs`

```rust
/// Глобальный tick counter (детерминистичный, wraparound safe)
///
/// Инкрементируется в каждый FixedUpdate tick (60 Hz).
/// Используется для запуска low-frequency schedules (SlowUpdate, CombatUpdate).
///
/// # Overflow Protection
/// u64::MAX / 60 / 60 / 60 / 24 / 365 ≈ 9.7 миллиардов лет.
/// Wraparound safe: modulo автоматически handle overflow.
#[derive(Resource, Default)]
pub struct FixedTickCounter {
    pub tick: u64,
}
```

#### Шаг 2.3: Создать CombatUpdate schedule

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs`

```rust
/// Custom schedule: SlowUpdate (3 Hz = 60/20)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct SlowUpdate;

/// Custom schedule: CombatUpdate (10 Hz = 60/6)
///
/// Для combat-критичных систем с быстрой реакцией:
/// - Windup detection (detect_melee_windups_main_thread)
/// - Combat events processing
/// - Timing-sensitive mechanics
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct CombatUpdate;
```

#### Шаг 2.4: Реализовать timer systems (exclusive)

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs`

```rust
/// System: Increment tick counter (FixedUpdate, запускается ПЕРВЫМ)
fn increment_tick_counter(mut counter: ResMut<voidrun_simulation::FixedTickCounter>) {
    counter.tick = counter.tick.wrapping_add(1);  // Wraparound safe
}

/// System: Run SlowUpdate schedule каждые 20 ticks (3 Hz @ 60 Hz fixed)
///
/// Exclusive system (требует &mut World для run_schedule)
fn run_slow_update_timer(world: &mut bevy::prelude::World) {
    let tick = world.resource::<voidrun_simulation::FixedTickCounter>().tick;

    if tick % 20 == 0 {
        world.run_schedule(SlowUpdate);
    }
}

/// System: Run CombatUpdate schedule каждые 6 ticks (10 Hz @ 60 Hz fixed)
///
/// Exclusive system (требует &mut World для run_schedule)
fn run_combat_update_timer(world: &mut bevy::prelude::World) {
    let tick = world.resource::<voidrun_simulation::FixedTickCounter>().tick;

    if tick % 6 == 0 {
        world.run_schedule(CombatUpdate);
    }
}
```

#### Шаг 2.5: Зарегистрировать schedules + systems

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs` (в `ready()`)

```rust
// 4.1b Создаём schedules + FixedTickCounter resource
app.init_schedule(SlowUpdate);
app.init_schedule(CombatUpdate);
app.insert_resource(voidrun_simulation::FixedTickCounter::default());

// 4.1c Timer systems в FixedUpdate (ВАЖНО: .chain() для порядка!)
app.add_systems(
    bevy::prelude::FixedUpdate,
    (
        increment_tick_counter,      // 1. Increment tick ПЕРВЫМ
        run_slow_update_timer,       // 2. Check SlowUpdate timer (exclusive)
        run_combat_update_timer,     // 3. Check CombatUpdate timer (exclusive)
    ).chain()
);

// 4.2 SlowUpdate systems (3 Hz)
app.add_systems(
    SlowUpdate,
    (
        poll_vision_cones_main_thread,
        update_combat_targets_main_thread,
    ).chain()
);

// 4.3 CombatUpdate systems (10 Hz) - пока пустой, добавим в Фазе 3
app.add_systems(CombatUpdate, (
    // detect_melee_windups_main_thread,  // TODO: Фаза 3
));
```

#### Шаг 2.6: Удалить старый SlowUpdateTimer

**Удалить:**
- `SlowUpdateTimer` resource definition
- `SlowUpdateTimer { timer: 0.0, interval: 0.3 }` insert
- Timer tick код в `process()` (строки 230-239)

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs`

---

### ФАЗА 3: Melee Combat Refactoring 🗡️

**Приоритет:** ВЫСОКИЙ (player combat support)

#### Шаг 3.1: Создать новый event GodotAIEvent::EnemyWindupVisible

**Файл:** `crates/voidrun_simulation/src/ai/events.rs`

```rust
/// Godot visual event: Enemy windup visible (spatial + angle detection)
///
/// Generated by `detect_melee_windups_main_thread` (Godot, 10 Hz) when:
/// - Attacker in Windup phase (MeleeAttackState)
/// - Defender within weapon range
/// - Attacker facing defender (60° cone, dot > 0.5)
/// - Defender visible (in attacker's SpottedEnemies)
///
/// Processed by AI combat decision system (defender decides parry/attack).
EnemyWindupVisible {
    /// Entity attacking (in Windup phase)
    attacker: Entity,
    /// Entity that can see windup (defender)
    defender: Entity,
    /// Time remaining in windup phase (seconds)
    windup_remaining: f32,
},
```

#### Шаг 3.2: Убрать target из MeleeAttackIntent

**Файл:** `crates/voidrun_simulation/src/combat/melee.rs`

```rust
// ❌ БЫЛО:
pub struct MeleeAttackIntent {
    pub attacker: Entity,
    pub target: Entity,  // ← УБРАТЬ!
    pub attack_type: MeleeAttackType,
}

// ✅ СТАЛО:
pub struct MeleeAttackIntent {
    pub attacker: Entity,
    // NO target - area-based detection
    pub attack_type: MeleeAttackType,
}
```

**Аналогично:**
- `MeleeAttackStarted` → убрать `target`
- `MeleeAttackState` → убрать `target`, `has_hit_target`

#### Шаг 3.3: Реализовать detect_melee_windups_main_thread

**Файл:** `crates/voidrun_godot/src/systems/weapon_system.rs`

```rust
/// System: Detect visible melee windups (CombatUpdate, 10 Hz)
///
/// For all actors in Windup phase:
/// - Spatial query: enemies within weapon range
/// - Angle check: attacker facing defender (60° cone)
/// - Visibility: defender in attacker's SpottedEnemies
/// - Emit: GodotAIEvent::EnemyWindupVisible
///
/// **AI реагирует на визуальные cues (реалистично)**
pub fn detect_melee_windups_main_thread(
    attackers: Query<(Entity, &Actor, &MeleeAttackState, &WeaponStats, &SpottedEnemies)>,
    defenders: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    mut ai_events: EventWriter<voidrun_simulation::ai::GodotAIEvent>,
) {
    const ANGLE_THRESHOLD: f32 = 0.5; // cos(60°) - hardcoded, балансинг позже

    for (attacker_entity, attacker_actor, attack_state, weapon, spotted) in attackers.iter() {
        // Только Windup phase
        if !attack_state.is_windup() {
            continue;
        }

        // Godot Transform (tactical layer)
        let Some(attacker_node) = visuals.visuals.get(&attacker_entity) else {
            continue;
        };

        let attacker_pos = attacker_node.get_global_position();
        let attacker_forward = attacker_node.get_global_transform().basis.col_c(); // +Z forward

        // Spatial query: все видимые враги в spotted
        for &defender_entity in &spotted.enemies {
            // Проверка faction (только враги)
            let Ok(defender_actor) = defenders.get(defender_entity) else {
                continue;
            };

            if defender_actor.faction_id == attacker_actor.faction_id {
                continue;
            }

            // Distance check
            let Some(defender_node) = visuals.visuals.get(&defender_entity) else {
                continue;
            };

            let defender_pos = defender_node.get_global_position();
            let distance = (defender_pos - attacker_pos).length();

            if distance > weapon.melee_range {
                continue;
            }

            // Angle check: attacker facing defender (60° cone)
            let to_defender = (defender_pos - attacker_pos).normalized();
            let dot = attacker_forward.dot(to_defender);

            if dot < ANGLE_THRESHOLD {
                continue; // Не смотрит на defender
            }

            // ✅ DEFENDER CAN SEE WINDUP!
            ai_events.write(voidrun_simulation::ai::GodotAIEvent::EnemyWindupVisible {
                attacker: attacker_entity,
                defender: defender_entity,
                windup_remaining: attack_state.phase_timer,
            });

            voidrun_simulation::log(&format!(
                "👁️ Windup visible: {:?} → {:?} (distance: {:.1}m, angle: {:.2}, windup: {:.2}s)",
                attacker_entity, defender_entity, distance, dot, attack_state.phase_timer
            ));
        }
    }
}
```

#### Шаг 3.4: Обновить AI combat decision

**Файл:** `crates/voidrun_godot/src/systems/ai_melee_combat_decision.rs`

**Изменения:**
1. Убрать обработку `CombatAIEvent::EnemyAttackTelegraphed`
2. Добавить обработку `GodotAIEvent::EnemyWindupVisible`
3. Logic аналогична (defender решает парировать или нет)

**Не трогать:**
- ParryIntent emission (работает как раньше)
- ParryState logic (использует `attacker` entity, не target)

#### Шаг 3.5: Обновить systems

**Файлы:**
- `process_melee_attack_intents_main_thread` - убрать target validation
- `start_melee_attacks` - убрать telegraph emission, MeleeAttackState без target
- `player_combat_input` - MeleeAttackIntent без target (уже так!)

**Удалить:**
- `CombatAIEvent::EnemyAttackTelegraphed` event definition

#### Шаг 3.6: Зарегистрировать систему

**Файл:** `crates/voidrun_godot/src/simulation_bridge.rs`

```rust
// CombatUpdate systems (10 Hz)
app.add_systems(CombatUpdate, (
    detect_melee_windups_main_thread,  // ← ДОБАВИТЬ
));
```

---

## Чеклист выполнения

### Фаза 1: Player Input ✅ ЗАВЕРШЕНА
- [x] Player component создан
- [x] Input module структура
- [x] Without<Player> фильтры в AI
- [x] Ошибки компиляции исправлены (deprecated methods: get_single→single, send→write)
- [x] Player combat временно отключён (заработает после Фазы 3)
- [x] Движение W/S исправлено (инверсия для Godot -Z convention)
- [x] Документация coordinate system добавлена
- [x] Safe нормализация подтверждена (defense in depth)
- [x] Компиляция успешна (готово к тестированию в Godot)

### Фаза 2: Schedule Refactoring ✅ ЗАВЕРШЕНА
- [x] FixedUpdate изменён с 64 Hz → 60 Hz (simulation + godot)
- [x] FixedTickCounter resource создан (voidrun_godot/schedules/mod.rs)
- [x] CombatUpdate schedule создан (voidrun_godot/schedules/mod.rs)
- [x] Timer systems реализованы (voidrun_godot/schedules/timer_systems.rs)
- [x] Systems зарегистрированы в FixedUpdate (.chain() для порядка)
- [x] SlowUpdateTimer удалён (старый код из process())
- [x] Протестировано (AI работает как раньше) ✅

### Фаза 3: Melee Combat Refactoring 🗡️
- [ ] GodotAIEvent::EnemyWindupVisible создан
- [ ] MeleeAttackIntent target убран
- [ ] MeleeAttackStarted target убран
- [ ] MeleeAttackState target убран
- [ ] detect_melee_windups_main_thread реализован
- [ ] AI combat decision обновлён
- [ ] Systems обновлены (start_melee_attacks, etc.)
- [ ] CombatAIEvent::EnemyAttackTelegraphed удалён
- [ ] Зарегистрировано в CombatUpdate
- [ ] Протестировано (AI parry работает)
- [ ] Протестировано (player combat работает)

---

## Риски и митигации

### Риск 1: Exclusive systems performance
**Проблема:** `&mut World` блокирует параллелизм
**Митигация:** Timer systems простые (одна проверка modulo), negligible cost

### Риск 2: Telegraph spam (все в радиусе)
**Проблема:** Много врагов → много events
**Митигация:** 10 Hz частота + spatial query дешёвая, можно профилировать позже

### Риск 3: Angle threshold balance
**Проблема:** 60° может быть слишком широко/узко
**Митигация:** Hardcoded сейчас, балансинг после тестов (позже в WeaponStats)

---

## Будущие улучшения

1. **WeaponStats balancing:**
   ```rust
   pub struct WeaponStats {
       pub detection_angle: f32,      // 60° → data-driven
       pub reaction_margin: f32,      // ±0.05s random для skill-based timing
   }
   ```

2. **Performance profiling:**
   - Измерить cost detect_melee_windups @ 10 Hz
   - Spatial query optimization (chunk-based?)

3. **Multi-target attacks:**
   - Sweep attacks (все в hitbox)
   - Cleave damage

---

## Примечания

- Все изменения обратно совместимы (AI работает как раньше)
- Player combat появляется автоматически (area-based detection)
- Детерминизм критичен для multiplayer (будущее)

**Версия:** 1.1
**Автор:** Claude Code + User
**Дата:** 2025-01-17
**Обновлено:** 2025-01-17 (Фаза 1 завершена)

---

## История изменений

### 2025-01-17 - Фаза 1 завершена (частично)

**Выполнено:**
- ✅ Player component создан (`voidrun_simulation/src/components/player.rs`)
- ✅ Input module структура (`voidrun_godot/src/input/{events.rs, systems.rs, controller.rs, mod.rs}`)
- ✅ PlayerInputController (Godot node, читает Input API)
- ✅ Player spawn helper (`voidrun_godot/src/player/spawn.rs`)
- ✅ "Spawn Player" button в UI
- ✅ Without<Player> фильтры добавлены в AI системы
- ✅ Ошибки компиляции исправлены:
  - `get_single()` → `single()` (deprecated в Bevy 0.16)
  - `send()` → `write()` (deprecated EventWriter API)
  - `glam::Vec2` → `Vec2` (импорт из bevy::prelude)
- ✅ Протестировать spawn player + WASD movement в игре - работает, но нужно инвертировать вперед - назад потому что щас W двигает актора в +Z что неправильно

**Временные решения:**
- ⚠️ Player combat (LMB attack) временно отключён - заработает после Фазы 3 (area-based melee refactor)
- Причина: MeleeAttackIntent.target требует Entity (не Option), будет убрано в Фазе 3

**Осталось:**


**Компиляция:** ✅ Успешно (3.29s, только warnings)

### 2025-01-18 - Фаза 1 ЗАВЕРШЕНА полностью ✅

**Выполнено:**
- ✅ Исправлена инверсия движения W/S (controller.rs)
  - W: `move_direction.y -= 1.0` → forward (Godot -Z convention)
  - S: `move_direction.y += 1.0` → backward (Godot +Z convention)
- ✅ Добавлена документация coordinate system (events.rs)
  - Logical direction vs Godot conventions
  - Примеры для всех клавиш (W/A/S/D)
- ✅ Safe нормализация подтверждена (defense in depth):
  - Controller: `if length() > 0.0 → normalized()`
  - System: `!is_nan() && length_squared() > 0.01`

**Тестирование:**
- Компиляция: ✅ 13.74s (только warnings)
- Player movement готов к тестированию в Godot

**Статус:** Фаза 1 ЗАВЕРШЕНА. Готово к Фазе 2 (Schedule Refactoring).

### 2025-01-18 - Фаза 2 ЗАВЕРШЕНА полностью ✅

**Выполнено:**
- ✅ FixedUpdate частота изменена: 64 Hz → 60 Hz
  - `voidrun_simulation/src/lib.rs` (2 места)
- ✅ Архитектура schedules: Tick-based вместо frame-dependent timers
  - **НЕ используем:** Bevy `on_timer()` (frame-dependent, дрейфует)
  - **Используем:** Tick counter + modulo (детерминистично, wraparound safe)
- ✅ Новые файлы созданы (избежали раздувания simulation_bridge.rs):
  - `voidrun_godot/src/schedules/mod.rs` - FixedTickCounter, SlowUpdate, CombatUpdate
  - `voidrun_godot/src/schedules/timer_systems.rs` - increment_tick, run_slow/combat_update
- ✅ Systems зарегистрированы:
  - FixedUpdate: `increment_tick_counter` → `run_slow_update_timer` → `run_combat_update_timer` (.chain())
  - SlowUpdate (3 Hz): vision cones, target switching
  - CombatUpdate (10 Hz): пустой пока (windup detection в Фазе 3)
- ✅ Старый код удалён:
  - `SlowUpdateTimer` resource + timer tick logic из `process()`
- ✅ **КРИТИЧНО:** Добавлено правило в CLAUDE.md
  - Файлы >750 строк → СТОП, архитектурное обсуждение
  - Максимум 950 строк (абсолютная граница)

**Тестирование:**
- Компиляция: ✅ Успешно (только warnings)
- AI работает: ✅ Vision, target switching работают как раньше

**Статус:** Фаза 2 ЗАВЕРШЕНА. Готово к Фазе 3 (Melee Combat Refactoring).

---
