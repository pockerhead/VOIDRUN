# ADR-003: ECS vs Godot Physics Ownership (Hybrid Architecture)

**Дата:** 2025-01-10
**Статус:** ✅ Принято
**Контекст:** Разделение ответственности между Bevy ECS simulation и Godot physics

---

## Контекст

После завершения Фазы 1 (Combat Core) возник критический вопрос архитектуры:

**Проблема:**
- Сейчас Rapier (Rust) + Godot Physics синхронизируются (дублирование)
- Transform в ECS + Transform в Godot (двойная бухгалтерия)
- Для systems-driven RPG важен **world state**, не точная физика

**Ключевой инсайт:**
> Для AI важен **decision** ("пойти в сектор X"), а не процесс ходьбы. Godot выполняет решение визуально.

**Варианты рассмотренные:**
1. **ECS Authoritative** — вся физика в Rapier, Godot = визуал (текущее)
2. **Godot Authoritative** — вся физика в Godot, ECS = только данные
3. **Hybrid** — ECS = strategic layer, Godot = tactical layer

---

## Решение

**Выбрано:** Вариант 3 (Hybrid) с уклоном в Godot

**Архитектура:**

```
┌─────────────────────────────────────────┐
│ Bevy ECS (Strategic Layer)              │
│ - Authoritative game state              │
│ - AI decisions, goals                   │
│ - Combat rules (damage calculation)     │
│ - World state (economy, factions)       │
│ - Strategic position (Zone/Sector)      │
└─────────────────────────────────────────┘
           ↓ commands ("do X")
           ↑ events ("X happened")
┌─────────────────────────────────────────┐
│ Godot (Tactical Layer)                  │
│ - Authoritative transform               │
│ - Physics (CharacterBody3D, collisions) │
│ - Combat execution (animation hitboxes) │
│ - Pathfinding (NavigationAgent3D)       │
│ - Visual effects, particles             │
└─────────────────────────────────────────┘
```

**Ключевые принципы:**
- ✅ ECS владеет **gameplay-critical state** (health, inventory, faction reputation)
- ✅ Godot владеет **precise transform** и физикой
- ✅ Синхронизация редкая (commands + events, 1-10Hz)
- ✅ ECS оперирует зонами/секторами (не точными координатами)

---

## Обоснование

### Почему Hybrid, а не ECS Authoritative?

**1. Single-player priority:**
- VOIDRUN = single-player systems RPG (Kenshi в космосе)
- Multiplayer = client-server authoritative (не P2P rollback)
- **Детерминизм не критичен** для core experience

**2. Systems-driven gameplay:**
- Важен **world state** (economy, factions, AI decisions)
- **Точная позиция не влияет** на системы (достаточно зоны/сектора)
- Примеры:
  - Economy: цены не зависят от того где стоит NPC (важен сектор)
  - AI: "атаковать врага" — не нужны точные координаты для decision
  - Quests: триггеры по зонам ("прибыл в систему X")

**3. Godot features бесплатно:**
- NavigationAgent3D (A\*, dynamic avoidance) — нет смысла писать свой
- AnimationTree (blend, state machines) — лучше чем код
- Physics layers (collision filtering) — визуальная настройка
- Editor tools (hitbox placement) — художник работает без программиста

**4. Меньше кода:**
- Не пишем свою физику (Rapier kinematic → убрать)
- Не синхронизируем Rapier ↔ Godot (было headache)
- Меньше Rust кода = быстрее к gameplay features

### Почему не Godot Authoritative (полностью)?

**Нельзя потерять:**
- ✅ Headless симуляция (для economy, AI, quest tests)
- ✅ Authoritative server (для будущего multiplayer)
- ✅ Deterministic saves (snapshot ECS state)
- ✅ Replay debugging (через event log)

**Решение:** Hybrid — ECS владеет game state, Godot владеет physics

---

## Детальный дизайн

### Что в ECS (Bevy):

**Authoritative Game State:**
```rust
#[derive(Component)]
struct Actor {
    faction_id: u32,
    visual_id: VisualId, // NOT Godot path
}

#[derive(Component)]
struct Health { current: f32, max: f32 }

#[derive(Component)]
struct Inventory { items: Vec<ItemId> }

#[derive(Component)]
struct StrategicPosition {
    zone: ZoneId,        // Coarse-grained (sector, region)
    approx_pos: Vec3,    // Updated from Godot 1Hz
}
```

**AI System (decision-making):**
```rust
#[derive(Component)]
enum AIState {
    Idle,
    Patrolling { zone: ZoneId },
    Attacking { target: Entity },
    Fleeing,
    Trading { destination: ZoneId },
}

// AI делает decision на основе coarse position
fn ai_decision_system(
    query: Query<(&StrategicPosition, &AIState, &Actor)>,
) {
    // Достаточно знать "я в зоне A, враг в зоне B"
    // НЕ нужны точные coordinates
}
```

**Combat Rules (data-driven):**
```rust
#[derive(Component)]
struct WeaponStats {
    damage: f32,
    range: f32,
    armor_penetration: f32,
    stamina_cost: f32,
    // Визуал = отдельно (VisualId)
}

// Damage calculation = ECS system
fn apply_damage_system(
    mut events: EventReader<DamageDealtEvent>, // From Godot
    mut query: Query<&mut Health>,
) {
    for event in events.read() {
        // ECS применяет урон (authoritative)
        health.current -= event.amount;
    }
}
```

**World Systems:**
```rust
// Economy simulation (НЕ зависит от точных positions)
fn economy_system(prices: Res<MarketPrices>, sectors: Query<&Sector>) {
    // Update prices based on supply/demand
}

// Faction AI (войны/мир)
fn faction_diplomacy_system() {
    // Decisions based on reputation, not positions
}
```

### Что в Godot:

**Transform Ownership:**
```gdscript
# CharacterBody3D owns Transform
var position: Vector3  # Authoritative in Godot
var velocity: Vector3
var rotation: Quaternion
```

**Combat Execution (animation-driven):**
```
CharacterBody3D (root)
├─ MeshInstance3D (visual)
├─ AnimationPlayer (controls weapon swing)
└─ WeaponHitbox (Area3D, child node)
    └─ CollisionShape3D (capsule)

# AnimationPlayer triggers:
# - Frame 10: enable WeaponHitbox collision
# - Frame 20: disable WeaponHitbox collision
```

**AI Execution:**
```rust
// Godot Rust code (SimulationBridge)
fn execute_ai_command(&mut self, cmd: AICommand) {
    match cmd {
        AICommand::MoveToZone(zone_id) => {
            let target_pos = self.get_zone_center(zone_id);

            // NavigationAgent3D handles pathfinding
            let nav_agent = character.get_node::<NavigationAgent3D>("NavAgent");
            nav_agent.set_target_position(target_pos);

            // CharacterBody3D handles movement
            // (в _physics_process)
        }
        AICommand::AttackTarget(entity_id) => {
            // Trigger attack animation
            let anim = character.get_node::<AnimationPlayer>("Anim");
            anim.play("sword_slash");

            // Hitbox activation = animation-driven
        }
    }
}
```

### Синхронизация (двусторонняя):

**ECS → Godot (Commands, high-level):**
```rust
enum SimulationCommand {
    // Spawn/despawn
    SpawnEntity {
        entity_id: u32,
        visual_id: VisualId,
        zone: ZoneId
    },
    DespawnEntity { entity_id: u32 },

    // AI commands (high-level goals)
    SetAIGoal {
        entity_id: u32,
        goal: AIGoal
    },

    // Combat events (triggers)
    TriggerDamage {
        entity_id: u32,
        amount: f32
    },
    PlayAnimation {
        entity_id: u32,
        anim_id: AnimId
    },
}

enum AIGoal {
    MoveToZone(ZoneId),
    AttackEntity(u32),
    Flee,
    Idle,
}
```

**Godot → ECS (Events, sampling):**
```rust
enum GodotEvent {
    // Combat events
    DamageDealt {
        attacker: u32,
        victim: u32,
        amount: f32,
        hit_location: HitLocation,
    },

    // AI events (completion)
    EntityArrivedAtZone {
        entity_id: u32,
        zone: ZoneId
    },

    // Lifecycle
    EntityDied {
        entity_id: u32,
        killer: Option<u32>
    },

    // State changes
    CombatStateChanged {
        entity_id: u32,
        in_combat: bool
    },

    // Periodic sync (1-10Hz)
    PositionUpdate {
        entity_id: u32,
        position: Vec3,  // Approx position
        zone: ZoneId,    // Current zone
    },
}
```

**Пример флоу (AI attack):**

```
1. ECS AI System:
   "NPC должен атаковать врага"
   → Command: SetAIGoal { entity_id: 5, goal: AttackEntity(10) }

2. Godot (SimulationBridge):
   Получает команду
   → NavigationAgent3D pathfinding к цели
   → CharacterBody3D движется
   → Когда в range → AnimationPlayer.play("attack")

3. Godot (Animation Event):
   Animation frame 15 → trigger "hitbox_active"
   → WeaponHitbox collision enabled
   → Area3D.body_entered signal

4. Godot → ECS:
   Event: DamageDealt { attacker: 5, victim: 10, amount: 25.0 }

5. ECS Combat System:
   Применяет урон (authoritative)
   → Health.current -= 25.0
   → Если health <= 0 → Command: PlayAnimation(10, "death")

6. Godot:
   Получает команду
   → AnimationPlayer.play("death")
   → После animation → Event: EntityDied { entity_id: 10 }
```

---

## Trade-offs (Risks & Mitigations)

### ❌ Что ТЕРЯЕМ:

**1. Детерminистичные replays (input replay):**
- **Риск:** Godot Physics недетерминистична → input replay невозможен
- **Mitigation:**
  - Checkpoint-based replays (snapshot ECS state)
  - Video recording (OBS-style для debugging)
  - Event log replay (high-level commands)
- **Вероятность проблемы:** LOW (input replay не критичен для single-player)

**2. P2P rollback netcode:**
- **Риск:** Невозможен без bit-perfect determinism
- **Mitigation:** Client-Server authoritative (подходит для MMORPG-style)
- **Вероятность проблемы:** NONE (rollback не нужен для твоего случая)

**3. Cross-platform bit-perfect saves:**
- **Риск:** Godot Physics может отличаться между платформами
- **Mitigation:** Saves хранят ECS state (health, inventory), не точные coords
- **Вероятность проблемы:** LOW (стратегическая позиция достаточна)

**4. Headless physics tests:**
- **Риск:** Нужен Godot runtime для physics simulation
- **Mitigation:**
  - ECS unit tests без физики (AI, economy, quests)
  - Integration tests с Godot headless mode
- **Вероятность проблемы:** MEDIUM (но acceptable)

### ✅ Что ПОЛУЧАЕМ:

**1. Меньше кода:**
- Не пишем Rapier kinematic controller
- Не синхронизируем Rapier ↔ Godot transforms
- **Экономия:** ~500-1000 lines Rust code

**2. Godot features бесплатно:**
- NavigationAgent3D (A\*, dynamic avoidance, steering)
- AnimationTree (blend spaces, state machines)
- Physics layers (collision filtering, raycasts)
- Editor tools (visual hitbox placement, debugging)

**3. Быстрая итерация:**
- Художник настраивает hitboxes в редакторе (не код)
- Tweaking без пересборки Rust (hot reload materials/animations)
- Визуальный debugging (Godot remote scene inspector)

**4. Проще networking (future):**
- Client-Server проще чем P2P rollback
- Подходит для persistent world (MMORPG-style)
- Snapshot interpolation (standard approach)

**5. Фокус на systems:**
- Больше времени на economy, AI, quests
- Меньше времени на low-level physics debugging

---

## Критерии успеха

**Как понять что решение работает:**

**1. Headless ECS tests проходят:**
```bash
cargo test -p voidrun_simulation
# AI, economy, quest systems работают без Godot
```

**2. Godot physics чувствуется правильно:**
- Combat timing responsive
- Pathfinding без застреваний
- Animations smooth

**3. Синхронизация редкая:**
- ECS → Godot commands: <100/sec (low bandwidth)
- Godot → ECS events: <50/sec (sampling)
- No frame-by-frame sync (acceptable latency)

**4. Save/Load работает:**
```rust
// Save
let snapshot = world.snapshot(); // ECS state only
save_to_file(snapshot);

// Load
let snapshot = load_from_file();
world.restore(snapshot); // Strategic positions
// Godot respawns entities at approx positions
```

**5. Можно добавить multiplayer:**
- Server runs ECS authoritative
- Clients send inputs → server validates → applies
- Server sends ECS state → clients interpolate Godot transforms

---

## План внедрения

### Фаза 1: Refactor SimulationBridge (3-5 дней)

**Задачи:**
1. Создать `SimulationCommand` и `GodotEvent` enums
2. Event queue система (ECS → Godot, Godot → ECS)
3. Убрать direct ECS access из Godot code (только через events)

**Deliverables:**
- `voidrun_simulation/src/bridge/commands.rs` — command definitions
- `voidrun_simulation/src/bridge/events.rs` — event definitions
- `voidrun_godot/src/event_bridge.rs` — event processing

### Фаза 2: Transform ownership → Godot (1-2 дня)

**Задачи:**
1. Убрать `Transform` component из ECS (для actors)
2. Добавить `StrategicPosition` component (zone + approx pos)
3. Godot owns CharacterBody3D.position (authoritative)
4. Периодический sync: Godot → ECS (1Hz position sampling)

**Deliverables:**
- `StrategicPosition` component
- Position sync system (Godot → ECS events)

### Фаза 3: Combat → animation-driven (2-3 дня)

**Задачи:**
1. Weapon hitboxes = Area3D child nodes (в Godot prefabs)
2. AnimationPlayer triggers collision enable/disable
3. Area3D.body_entered → DamageDealt event → ECS
4. ECS damage system применяет урон (authoritative)

**Deliverables:**
- Prefabs с weapon hitboxes
- Animation event integration
- Damage event flow

### Фаза 4: AI → hybrid execution (2-3 дня)

**Задачи:**
1. ECS AI FSM → high-level goals (MoveToZone, AttackEntity)
2. Godot executes goals (NavigationAgent3D, CharacterBody3D)
3. Godot reports completion (ArrivedAtZone event)
4. ECS AI reacts (state transitions)

**Deliverables:**
- AI command system
- NavigationAgent3D integration
- AI event loop

### Фаза 5: Physics cleanup (1 день)

**Задачи:**
1. Убрать Rapier из `voidrun_simulation` (для movement)
2. Опционально: оставить Rapier queries (если нужны raycasts в ECS)
3. Update Cargo.toml (optional bevy_rapier dependency)

**Timeline:** 9-14 дней total → к концу января

---

## Откат (Plan B)

**Если Hybrid не зайдёт:**

**Сигналы что нужен откат:**
- Godot physics нестабильна (bugs, crashes, performance issues)
- Синхронизация ECS ↔ Godot = bottleneck (>100ms latency)
- Headless tests критично нужны для physics (CI requirement)
- Multiplayer требует P2P rollback (неожиданное требование)

**План возврата к ECS Authoritative:**
1. Вернуть `Transform` component в ECS
2. Вернуть Rapier kinematic controller
3. Godot читает Transform read-only (как было в Фазе 1)
4. **Стоимость:** 5-7 дней работы (код уже был, восстановить)

**Риск:** LOW (Godot Physics mature, unlikely к провалу)

---

## Связанные решения

- [ADR-001: Godot vs Bevy для визуализации](ADR-001-godot-vs-bevy.md) — почему Godot
- [ADR-002: Godot-Rust Integration Pattern](ADR-002-godot-rust-integration-pattern.md) — SimulationBridge
- [physics-architecture.md](../architecture/physics-architecture.md) — оригинальный дизайн (ECS authoritative)

---

## Research Links

**Authoritative Server vs P2P:**
- [Gaffer On Games: State Synchronization](https://gafferongames.com/post/state_synchronization/)
- Photon Quantum: deterministic ECS для rollback (не нужно для client-server)

**Hybrid Architectures:**
- Unity DOTS + GameObjects hybrid (similar approach)
- Unreal: Gameplay Ability System (data in C++, execution in Blueprint)

**Single-player vs Multiplayer Physics:**
- Single-player: визуальная stабильность > bit-perfect determinism
- Multiplayer client-server: snapshot interpolation (standard)

---

**Итого:** ECS = brain (decisions, world state), Godot = body (physics, animations). Делаем **игру**, не борьбу с физикой.
