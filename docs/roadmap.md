# VOIDRUN Development Roadmap

**Версия:** 1.0
**Обновлено:** 2025-01-07
**Стратегия:** Headless-first (70%) + Debug визуал (30%)

---

## ✅ Фаза 0: Foundation (ЗАВЕРШЕНО)

**Срок:** 1 день
**Статус:** ✅ Completed

### Цели:
- Workspace структура с оптимальными зависимостями
- Детерминизм proof-of-concept
- FMA disabled для кросс-CPU совместимости

### Достижения:
- ✅ Bevy 0.16 MinimalPlugins (headless)
- ✅ DeterministicRng (ChaCha8Rng)
- ✅ 64Hz FixedUpdate schedule
- ✅ 2 детерминизм-теста (100 entities, 1000 тиков)
- ✅ bevy_rapier3d 0.31 проверен (2:44 build time)
- ✅ Компиляция: 2 сек incremental, 41 сек full build (без физики)

### Решения:
- Без физики в Фазе 0 → быстрая итерация
- bevy_rapier 0.31 готов для Фазы 1
- SIMD отключен (конфликт с enhanced-determinism)

---

## ✅ Фаза 1: Physics + Combat Core (ЗАВЕРШЕНО)

**Срок:** 2-3 недели
**Статус:** ✅ Completed (2025-01-09)

### Milestone цель:
**2 NPC дерутся headless 1000 тиков без крашей, детерminистично** ✅

### Достижения:

**Physics Foundation:**
- ✅ bevy_rapier3d 0.31 интегрирован (default-features = false, headless-friendly)
- ✅ Компоненты: Actor, Health, Stamina, PhysicsBody, KinematicController
- ✅ Movement: velocity integration (прямая, rapier только для collisions)
- ✅ Capsule коллайдеры для actors (radius 0.4m, height 1.8m)
- ✅ Collision groups: actors не коллайдят между собой, weapons детектят hits

**Combat System:**
- ✅ Weapon hitbox: меч-капсула 1.5m длиной, child entity, rapier Sensor
- ✅ Swing animation: diagonal slash (-30° → -120° pitch, 0.2s duration)
- ✅ Damage system: base damage × stamina multiplier
- ✅ Stamina: attack cost 30, regen 10/sec, exhaustion при 0
- ✅ Collision detection: weapon swing → rapier CollisionEvent → DamageDealt

**AI System:**
- ✅ Simple FSM: Idle → Aggro → Approach → Attack → Retreat
- ✅ Target detection: faction-based, 10m radius
- ✅ Movement: AI → MovementInput → velocity → transform
- ✅ Attack execution: stamina check, cooldown, AttackStarted events

**Godot Visualization:**
- ✅ 100% Rust visuals (no GDScript)
- ✅ Health bar (над головой)
- ✅ Stamina bar (зелёная, под health)
- ✅ AI state label (желтая, над health)
- ✅ Weapon mesh: длинная капсула, диагональная поза, swing animation sync
- ✅ Hit particles: красные сферы при damage
- ✅ RTS camera: WASD pan, RMB orbit, scroll zoom

### Тесты пройдены:
- ✅ `cargo test combat_integration` — 3/3 passed (1000 ticks, determinism, invariants)
- ✅ `cargo test determinism` — 2/2 passed (same seed, multiple runs)
- ✅ 28 unit tests — all passed
- ✅ Godot runtime: 2 NPC дерутся, видны все визуалы

### Технические решения:
- **Rapier роль:** только collision detection (weapon hits), движение через direct integration
- **Collision groups:** actors проходят друг через друга (Group::NONE), weapons детектят actors
- **Determinism:** 64Hz fixed timestep, ChaCha8Rng, ordered systems
- **Architecture:** Rust simulation полностью independent, Godot = presentation layer

### Deliverables:
- ✅ `voidrun_simulation/src/physics/movement.rs` — kinematic controller
- ✅ `voidrun_simulation/src/combat/weapon.rs` — weapon system (340+ lines)
- ✅ `voidrun_simulation/src/combat/damage.rs` — damage calculation
- ✅ `voidrun_simulation/src/combat/stamina.rs` — stamina management
- ✅ `voidrun_simulation/src/ai/simple_fsm.rs` — AI FSM (350+ lines)
- ✅ `voidrun_godot/src/simulation_bridge.rs` — Godot visualization (400+ lines)
- ✅ `tests/combat_integration.rs` — integration tests

---

## 🚧 Фаза 1.5: Combat Mechanics + Player Control (ТЕКУЩЕЕ)

**Срок:** 5-8 дней
**Статус:** 🎯 In progress
**Обновлено:** 2025-01-10

### Milestone цель:
**Playable prototype: игрок дерётся с NPC (ближний + дальний бой)**

### Зачем:
- 🎮 Сделать игру играбельной (сейчас только NPC vs NPC)
- 🎮 Проверить combat feel (timing, weight, impact)
- 🎮 Foundation для inventory/loot систем
- 🎮 Fun factor > 0 перед переходом к экономике

### Архитектурные решения (2025-01-10):
- ✅ **ADR-002:** Godot-Rust Integration (SimulationBridge без abstraction, YAGNI)
- ✅ **ADR-003:** Hybrid Architecture (ECS strategic + Godot tactical physics)
- ✅ **ADR-004:** Command/Event Architecture (Bevy Events, Changed<T> sync)
- ✅ **ADR-005:** Transform Ownership (Godot Transform + ECS StrategicPosition)
- ✅ **ADR-006:** Chunk-based Streaming World (procgen, seed + deltas saves)
- ✅ **Assets:** Godot prefabs + Rust load через `load::<T>("res://")`

### Задачи:

**Player Control (1-2 дня):**
- [ ] WASD movement через Godot Input events
- [ ] Mouse attack (LMB → swing weapon)
- [ ] Camera follow player (3rd person, smooth)
- [ ] Health/Stamina HUD (bars на screen space)

**Melee Combat Polish (1 день):**
- [ ] Parry window (200ms timing, perfect block = no damage)
- [ ] Block action (hold RMB, stamina drain 5/sec)
- [ ] Dodge roll (spacebar, i-frames 300ms, stamina cost 20)

**Ranged Combat System (2-3 дня):**
- [ ] Projectile physics (RigidBody3D с gravity)
- [ ] Bow/crossbow weapon type
- [ ] Ballistics (arc trajectory, deterministic)
- [ ] Ammo system (simple counter, pickup later)
- [ ] Ranged damage system (hit detection)

**AI Upgrade (1-2 дня):**
- [ ] Pathfinding (A* через Godot NavigationAgent3D)
- [ ] Ranged AI behavior (keep distance 5-10m, shoot)
- [ ] Dodge projectiles (simple raycast prediction)

### Фаза 1.5.5: Chunk System & Procgen Foundation (ДОБАВЛЕНО 2025-01-10)

**Срок:** 6-10 дней (параллельно с Combat Mechanics или после)
**Статус:** 📋 Planned

**Зачем:**
- 🌍 Процедурная генерация (нет ресурсов на ручные уровни)
- 🌍 Infinite world (Minecraft-style streaming chunks)
- 🌍 Компактные saves (seed + deltas, не full snapshot)
- 🌍 MMO-ready architecture

**Задачи (см. ADR-006 план имплементации):**

**Фаза 1: Chunk System Core (2-3 дня):**
- [ ] ChunkCoord (IVec2), ChunkData, LoadedChunks types
- [ ] `update_chunk_loading` система (load radius вокруг игрока)
- [ ] Простейшая procgen (один биом, детерминированный RNG)
- [ ] ChunkEvent::Load/Unload

**Фаза 2: Godot Integration (1-2 дня):**
- [ ] `process_chunk_events` (geometry loading/unloading)
- [ ] `spawn_entities_in_loaded_chunks` (NPC spawn на NavMesh)
- [ ] Chunk prefabs (corridor, warehouse scenes)

**Фаза 3: Procgen Content (2-3 дня):**
- [ ] Биомы (5-7 типов комнат: corridor, warehouse, reactor, medbay)
- [ ] Perlin noise для biome distribution
- [ ] Детерминированная генерация врагов/лута (RNG per chunk seed)

**Фаза 4: Save/Load (1-2 дня):**
- [ ] SaveFile (seed + player + chunk deltas)
- [ ] `calculate_chunk_delta` (diff от procgen baseline)
- [ ] Load с delta application

**Deliverables:**
- ✅ `docs/decisions/ADR-006` — Chunk-based Streaming World design
- `voidrun_simulation/src/world/chunk.rs` — chunk management
- `voidrun_simulation/src/world/procgen.rs` — procedural generation
- `voidrun_simulation/src/save/mod.rs` — seed + delta saves
- `voidrun_godot/src/world/chunk_loader.rs` — geometry loading

---

### Deliverables (общие для Фазы 1.5):

**Architecture:**
- ✅ `docs/decisions/ADR-002` — Godot-Rust Integration Pattern
- ✅ `docs/decisions/ADR-003` — ECS vs Godot Physics Ownership
- ✅ `docs/decisions/ADR-004` — Command/Event Architecture (Bevy Events)
- ✅ `docs/decisions/ADR-005` — Transform Ownership & Strategic Positioning
- ✅ `docs/decisions/ADR-006` — Chunk-based Streaming World (Procgen)
- `voidrun_simulation/src/events.rs` — GodotInputEvent enum
- `voidrun_simulation/src/components.rs` — StrategicPosition component

**Gameplay:**
- `voidrun_simulation/src/player/` — player control systems (ECS)
- `voidrun_simulation/src/combat/projectile.rs` — projectile rules (data)
- `voidrun_godot/src/player_input.rs` — input handling (Godot)
- `voidrun_godot/src/combat_execution.rs` — animation-driven combat
- `godot/assets/prefabs/` — character/weapon prefabs
- Playable demo: 1 player vs 2-3 NPC (mix melee/ranged)

### Checkpoint:
- ✅ Combat чувствуется (не "флэтовый")
- ✅ Dodge/parry timing работает (skill-based)
- ✅ AI не тупит (pathfinding без застреваний)
- ✅ Можно играть 5 минут без скуки

---

## 📋 Фаза 1.5.5: Presentation Layer Abstraction (POSTPONED - YAGNI)

**Статус:** ⏸️ Отложено (не нужно сейчас)
**Решение:** 2025-01-10

### Почему отложено:
- **YAGNI:** PresentationClient trait решает проблему которой нет
- **Godot работает:** SimulationBridge hybrid pattern — правильная архитектура
- **Фокус на геймплей:** 5-8 дней лучше потратить на player control + combat
- **Риск <5%:** смена рендера до 2026 = маловероятна

### Когда вернуться:
- Если появится реальная нужда в моддинг API
- Если захочется web/mobile render
- После Vertical Slice (когда есть что показать)

**Подробности:** См. ADR-002 (Godot-Rust Integration Pattern)

---

## 📋 Фаза 2: Save/Load System (REPLANNED)

**Срок:** 1-2 недели
**Статус:** 🔜 После Фазы 1.5
**Изменение:** Сначала single-player (save/load), потом netcode

### Milestone цель:
**Сохранение/загрузка боя mid-combat, детерминистичный replay**

### Зачем раньше netcode:
- Single-player priority (твоё решение)
- Save/load = foundation для netcode snapshot
- Проще тестировать детерминизм
- Replays = debugging tool

### Задачи:
- [ ] Snapshot system: serialize world state → bytes
- [ ] Deterministic serialization (ordered entities, components)
- [ ] Save/Load API: save_game(path), load_game(path)
- [ ] Replay system: record inputs → playback
- [ ] Tests: save → load → compare snapshots
- [ ] Godot UI: save/load menu (simple)

### Checkpoint:
- ✅ Можно сохранить mid-combat, загрузить → идентичное продолжение
- ✅ Replay 1000 ticks → битва повторяется детерминистично
- ✅ Save/Load < 100ms (performance acceptable)

---

## 📋 Фаза 3: Client-Server Netcode (POSTPONED)

**Срок:** 2-3 недели
**Статус:** 🔜 После Save/Load
**Изменение:** P2P rollback → Client-Server authoritative

### Решение (на основе обсуждения):
- **НЕ** P2P rollback — не подходит для MMORPG-style
- **ДА** Authoritative server + dumb clients
- Локальный server mode для single-player
- Dedicated server для multiplayer

### Задачи:
- [ ] Network protocol (Commands/Events)
- [ ] Local server thread (IPC с client)
- [ ] Serialization через presentation events
- [ ] Dedicated server binary (headless)
- [ ] Client connects via UDP

### Риски отложены до Фазы 3

---

## 📋 Фаза 2.5: Inventory + Loot (NEW)

**Срок:** 1 неделя
**Статус:** 🔜 После Combat Mechanics

### Milestone цель:
**Reward loop: kill NPC → loot items → equip better gear**

### Задачи:
- [ ] Inventory system (grid-based, capacity limit)
- [ ] Item definitions (weapons, armor, consumables)
- [ ] Loot drops (NPC death → spawn items)
- [ ] Equipment system (equip weapon/armor)
- [ ] Simple UI (inventory panel, drag-drop)

### Checkpoint:
- ✅ Можно подобрать items
- ✅ Equip влияет на stats (damage, defense)
- ✅ Reward loop работает (motivation играть)

---

## 📋 Фаза 3: Living Economy (PLANNING)

**Срок:** 2-3 недели
**Статус:** 🔜 После Фазы 2

### Milestone цель:
**Цены в 10 секторах сходятся за 100h headless, NPC traders живут своей жизнью**

### Задачи:
- [ ] Item definitions (RON format, ~20-30 товаров)
- [ ] Supply/Demand модель (производство, потребление, storage)
- [ ] NPC trader agents (autonomous, profit-driven)
- [ ] Dynamic trade routes (A* pathfinding, avoid danger)
- [ ] Price shock events (пираты, блокады)
- [ ] Background simulation (AISchedule 1Hz, работает всегда)
- [ ] Headless 100h galaxy run в CI

**Inspiration:** Space Rangers 2 economy, X4 supply chains

**Пятницы:** визуал опционален
- CLI графики цен (plotters crate → PNG)
- Sector map с trade routes
- Или CSV → Google Sheets

### Checkpoint:
- ✅ Цены не уходят в infinity/zero (property-тесты)
- ✅ Supply shock → восстановление за ~10h
- ✅ Traders избегают опасные сектора (pathfinding работает)
- ✅ Игрок видит последствия действий (убил trader → route изменился)

### Deliverables:
- `voidrun_simulation/src/economy/` — supply/demand, prices
- `voidrun_simulation/src/traders/` — NPC trader AI
- `data/items.ron` — item definitions
- `tests/economy_convergence.rs` — 100h headless test

---

## 📋 Фаза 4: Living World (Factions + Reputation) (PLANNING)

**Срок:** 3-4 недели
**Статус:** 🔜 После Фазы 3

### Milestone цель:
**3 фракции + личные NPC relationships, emergent stories работают**

### Задачи:

**Reputation System:**
- [ ] Faction reputation (HashMap<FactionId, i32>)
- [ ] Personal NPC bonds (trust, memorable events, relationship type)
- [ ] Reputation propagation (действие влияет на связанных NPC)
- [ ] Consequences: prices, quest availability, aggression, bounties

**NPC Progression (SR2-inspired):**
- [ ] NPC могут менять статус (trader → guild master)
- [ ] Emergent rivalry (другой ranger стал pirate leader)
- [ ] Player видит последствия ("Спасенный NPC дает скидку")

**Faction AI:**
- [ ] Faction goals и strategies
- [ ] Territory control
- [ ] Alliance/war declarations
- [ ] Resource management

**Background Simulation:**
- [ ] Мир живет без игрока (NPC выполняют квесты)
- [ ] Player может вернуться → сектор изменился
- [ ] Consequence chains (saved trader → becomes leader → affects world)

**Inspiration:** Space Rangers 2 (living galaxy), Mount & Blade (reputation), Kenshi (NPC progression)

### Checkpoint:
- ✅ Reputation влияет на gameplay (prices ±50%, quest access)
- ✅ NPC progression работает (минимум 3 примера emergent stories за 10h)
- ✅ Фракции принимают осмысленные решения (война/мир логичны)
- ✅ Игрок чувствует impact ("Мир реагирует на мои действия")

### Deliverables:
- `voidrun_simulation/src/reputation/` — faction + personal system
- `voidrun_simulation/src/factions/` — faction AI, relations
- `voidrun_simulation/src/npc_progression/` — status changes, events
- `tests/emergent_stories.rs` — тест consequence chains

---

## 🎯 Milestone: Vertical Slice (После Фазы 2)

**Что есть:**
- ✅ PvP бой 1v1 по сети
- ✅ Детерминизм доказан тестами
- ✅ Debug визуал показывает концепт

**Решение:**
- Делать ли полноценную Godot интеграцию?
- Или остаться на Bevy (если debug render зашел)?
- Показать концепт тестерам/инвесторам?

---

## 📋 Будущие фазы (После Vertical Slice)

### Фаза 5: Space Flight & Combat
- 6DOF полет
- Dogfight 1v1
- Transitions планета ↔ космос

### Фаза 6: Quests & Narrative
- Event-driven FSM для квестов
- Флаги и прогресс
- Procedural quest generation

### Фаза 7: Full Godot Integration
- Custom bridge (вместо godot-bevy)
- Полноценные модели и анимации
- UI/UX polish

### Фаза 8: Content Expansion
- 100+ items
- 50+ NPC archetypes
- 20+ ship types
- Procedural generation

---

## 🔄 Итерационная стратегия

**Каждая фаза:**
1. Headless core (80% времени)
2. Property-тесты и инварианты
3. Debug визуал по пятницам (20% времени)
4. Checkpoint перед переходом к следующей

**Философия:**
- Детерминизм > красота
- Системы > контент (на раннем этапе)
- Измеряй, не гадай (profiling, metrics)
- YAGNI — не пиши код "на будущее"

---

## 📊 Метрики успеха

**После Фазы 1:**
- Combat чувствуется как STALKER/Dishonored (timing, weight)

**После Фазы 2:**
- 10+ тестеров играют по сети без жалоб на лаги

**После Фазы 3:**
- Экономика "живая" — цены реагируют на действия игрока

**После Vertical Slice:**
- Ясно видно "душу игры" — что отличает от других space RPG

---

**Следующий шаг:** Начать Фазу 1 → добавить bevy_rapier3d и базовые компоненты.
