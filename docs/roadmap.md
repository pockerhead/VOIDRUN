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

## 🚧 Фаза 1.5: Presentation Layer Abstraction (СЛЕДУЮЩЕЕ)

**Срок:** 3-5 дней
**Статус:** 🎯 Next priority (начать завтра)
**Обновлено:** 2025-01-09

### Milestone цель:
**Simulation полностью independent от Godot через PresentationClient trait**

### Зачем:
- Чистая архитектура: "ассеты на Godot, крутим-вертим на Rust"
- Headless testing без Godot dependencies
- Моддинг: custom рендеры от community
- Гибкость: Bevy/web renderer в будущем

### Задачи:
- [ ] `presentation` module: PresentationClient trait + PresentationEvent enum
- [ ] Event system: simulation → event queue → client
- [ ] GodotPresentationClient (refactor SimulationBridge)
- [ ] HeadlessPresentationClient (no-op для tests)
- [ ] Update tests: HeadlessClient вместо direct ECS

### Deliverables:
- `voidrun_simulation/src/presentation/` — trait + events
- `voidrun_godot/src/godot_client.rs` — Godot impl
- Simulation без godot dependency ✅

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
