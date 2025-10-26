# VOIDRUN Development Roadmap

**Версия:** 1.3
**Обновлено:** 2025-10-26
**Стратегия:** Headless-first (70%) + Debug визуал (30%)

**Текущий фокус:** Chunk System & Procgen Foundation (Phase 3)

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

## ✅ Фаза 1.5: Combat Mechanics (ЗАВЕРШЕНО)

**Срок:** 3-5 дней
**Статус:** ✅ Melee combat system + Parry system fully implemented
**Обновлено:** 2025-10-15

**📋 Детальный план:** [Melee Combat Implementation](implementation/melee-combat-system.md)

### Milestone цель:
**NPC vs NPC combat (melee + ranged) полностью работает**

### Текущий статус (реальный):

**✅ Что РАБОТАЕТ:**
- ✅ Ranged combat: AI стреляет, projectiles летят, collision detection работает
- ✅ AI FSM: Idle → Patrol → Combat → Retreat
- ✅ Vision system: SpottedEnemies, ActorSpotted/Lost events
- ✅ Movement: MovementCommand, pathfinding (NavigationAgent3D)
- ✅ Godot visualization: health bars, projectiles, AI state labels
- ✅ Weapon attachment: test_pistol.tscn prefab система работает
- ✅ Tactical validation: distance/LOS checks (Godot Transform)

**✅ Melee Combat РАБОТАЕТ:**
- ✅ `ai_melee_attack_intent` система (генерирует атаки в Combat state)
- ✅ Melee hitbox collision detection (Area3D polling)
- ✅ Melee animation trigger система (windup → active → recovery phases)
- ✅ `MeleeHit` event → `DamageDealt` flow
- ✅ Anti-spam защита (`has_hit_target` flag — один хит на атаку)
- ✅ Реакция на урон (`react_to_damage` — разворот к атакующему)
- ✅ Тактическое отступление (`RetreatFrom` — backpedal + face target)
- ✅ Возврат в бой после Retreat (сохраняет `from_target`, не теряет врага)

**✅ Parry System РАБОТАЕТ (Фаза 2.2):**
- ✅ `ParryState` компонент (Windup → Recovery фазы)
- ✅ `StaggerState` компонент (ошеломление после успешного парирования)
- ✅ Critical timing check: парирование успешно если defender.Windup заканчивается когда attacker в `ActiveParryWindow`
- ✅ Attack phases расширены: `ActiveParryWindow` (hitbox OFF, можно парировать) + `ActiveHitbox` (hitbox ON, урон)
- ✅ AI парирует атаки с реакцией (`ParryDelayTimer` для realistic timing)
- ✅ `execute_parry_animations_main_thread` (melee_parry/melee_parry_recover)
- ✅ `execute_stagger_animations_main_thread` (RESET on stagger)

**✅ AI Decision Making:**
- ✅ `ai_melee_combat_decision_main_thread` (unified attack/parry/wait decisions)
- ✅ SlowUpdate schedule (0.3 Hz для AI систем с "временем реакции")
- ✅ Dynamic target switching (AI атакует ближайшего ВИДИМОГО врага с LOS check)
- ✅ `update_combat_targets_main_thread` (SlowUpdate) - LOS raycast для видимости

**✅ Performance Optimization:**
- ✅ poll_vision_cones: 60 Hz → 3 Hz (20x оптимизация)
- ✅ update_combat_targets: 60 Hz → 3 Hz (20x оптимизация LOS raycasts)
- ✅ **Результат: 21 NPC в бою @ 118-153 FPS** (стабильно)

**✅ Navigation & Movement:**
- ✅ NavigationAgent-based movement (LOS clearing, avoidance)
- ✅ `collision_layers.rs` (COLLISION_MASK_RAYCAST_LOS)
- ✅ `los_helpers.rs` (raycast utilities)
- ✅ `avoidance_receiver.rs` (NavigationAgent3D velocity_computed signal)

**✅ Player Control РАБОТАЕТ (2025-10-24):**
- ✅ FPS camera (mouse look: yaw body, pitch CameraPivot)
- ✅ Camera toggle [V] key (FPS ↔ RTS)
- ✅ WASD movement (camera-relative через Actor basis)
- ✅ Sprint (Shift: 6.0 м/с vs 3.0 м/с), Jump (Space)
- ✅ Combat input (LMB attack, RMB parry/ADS context-dependent)
- ✅ ADS system (Hip Fire ↔ ADS smooth transitions 0.3s)
- ✅ Weapon switch (Digit1-9 slots)
- ✅ PlayerInputController (Godot Node → ECS events)
- ✅ Gravity system для player

**📋 Player Polish (НЕ ГОТОВО):**
- ❌ Player HUD (health/stamina/ammo UI)
- ❌ Crosshair + damage feedback visuals
- ❌ Auto-spawn на старте сцены (сейчас через кнопку)
- ❌ Player death handling (respawn/game over)

**📋 Что НЕ НАЧАТО:**
- ⏸️ Shield system (design doc готов, code нет)
- ⏸️ Block/Dodge systems (парирование работает, блок/уклонение отложены)
- ⏸️ Chunk system (можем отложить)

### Архитектурные решения (2025-01-10):
- ✅ **ADR-002:** Godot-Rust Integration (SimulationBridge без abstraction, YAGNI)
- ✅ **ADR-003:** Hybrid Architecture (ECS strategic + Godot tactical physics)
- ✅ **ADR-004:** Command/Event Architecture (Bevy Events, Changed<T> sync)
- ✅ **ADR-005:** Transform Ownership (Godot Transform + ECS StrategicPosition)
- ✅ **ADR-006:** Chunk-based Streaming World (procgen, seed + deltas saves)
- ✅ **ADR-007:** TSCN Prefabs + Dynamic Attachment
- ✅ **Design Doc:** Shield Technology (Kinetic Threshold Barriers)

### Задачи (приоритет):

**✅ ЗАВЕРШЕНО: Weapon Architecture Refactoring (2025-01-13):**
- [x] Создан `WeaponStats` unified component (melee + ranged)
- [x] Удалён `Attacker` + старый `Weapon` struct
- [x] Рефакторинг ECS систем (`ai_weapon_fire_intent`, `ai_attack_execution`)
- [x] Рефакторинг Godot систем (`movement_system`, `simulation_bridge`)
- [x] `cargo test` компилируется без ошибок

**✅ ЗАВЕРШЕНО: Melee Combat Core (Фаза 2.1, 2025-10-14):**
- [x] `MeleeAttackIntent` event (ECS strategic decision)
- [x] `ai_melee_attack_intent` система (генерирует intent когда AI в Combat + близко)
- [x] `process_melee_attack_intents` система (Godot tactical validation)
- [x] `MeleeAttackStarted` event (ECS → Godot)
- [x] Melee weapon hitbox (Area3D collision detection)
- [x] Melee animation trigger (Godot AnimationPlayer)
- [x] `MeleeHit` event → `DamageDealt` (Godot → ECS damage)
- [x] `react_to_damage` система (автоматическая реакция на урон)
- [x] `RetreatFrom` movement command (тактическое отступление)
- [x] Правильная дистанция для melee/ranged (без буфера для melee)
- [x] Возврат в бой после Retreat (сохранение `from_target` в SpottedEnemies)

**✅ ЗАВЕРШЕНО: Parry System (Фаза 2.2, 2025-10-15):**
- [x] `ParryState` component (Windup → Recovery phases)
- [x] `StaggerState` component (ошеломление 0.5s после успешного парирования)
- [x] `ParryDelayTimer` component (AI reaction timing)
- [x] `ParryIntent` / `ParrySuccess` events
- [x] Attack phases расширены: `ActiveParryWindow` + `ActiveHitbox`
- [x] ECS systems: `start_parry`, `update_parry_states`, `update_stagger_states`, `process_parry_delay_timers`
- [x] Godot systems: `execute_parry_animations_main_thread`, `execute_stagger_animations_main_thread`
- [x] Critical timing check (defender.Windup ends когда attacker в ActiveParryWindow)
- [x] AI парирует атаки (`ai_melee_combat_decision_main_thread`)

**✅ ЗАВЕРШЕНО: Performance & AI Improvements (2025-10-15):**
- [x] SlowUpdate schedule (0.3 Hz для AI decision making)
- [x] Dynamic target switching (`update_combat_targets_main_thread`)
- [x] NavigationAgent-based movement (LOS clearing)
- [x] Collision layers система (`collision_layers.rs`, `los_helpers.rs`)
- [x] Performance: 21 NPC @ 118-153 FPS (poll_vision 60Hz→3Hz, target_switch 60Hz→3Hz)

**🎯 Shield System Implementation (2-3 дня):**
- [ ] `Shield` component (energy, threshold, regen_rate)
- [ ] Shield vs Damage system (ranged разряжает, melee игнорирует)
- [ ] Shield models (Military/Commercial/Civilian/Legacy с разными stats)
- [ ] Shield визуализация (мерцание при попадании, energy bar)
- [ ] Shield regeneration (вне боя)
- [ ] Balance tests (симуляция NPC боёв)

**⏸️ ОТЛОЖЕНО (можем сделать позже):**
- [ ] Player HUD polish (crosshair, damage feedback, death screen)
- [ ] Block system (блокирование с stamina drain, 70% damage reduction)
- [ ] Dodge system (i-frames, dash movement)
- [ ] Chunk system + procgen
- [ ] VATS system (design doc готов, implementation позже)
- [ ] Dialogue camera (cinematic shots)

### 🎯 Фаза 1.5.5: Chunk System & Procgen Foundation (СЛЕДУЮЩАЯ)

**Срок:** 6-10 дней
**Статус:** 🔜 NEXT (после Phase 2 Domain Refactoring)
**Обновлено:** 2025-10-26

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

### Фаза 5: Campaign & Narrative Systems (НОВАЯ ПРИОРИТЕТНАЯ ФАЗА)

**Срок:** 6-10 недель
**Статус:** 📋 Planned (design docs готовы)
**Философия:** Sandbox-first development

**📋 Детальные планы:**
- [Campaign & Sandbox System Design](design/campaign-sandbox-system.md)
- [Procedural Narrative System](design/procedural-narrative.md)
- [Endgame Systems](design/endgame-systems.md)

#### Phase 5.1: Core Sandbox Systems (Foundation) — 2-3 недели

**Milestone:** World generation с deep configuration работает

**Задачи:**

1. **World Generation (seed-based):**
   - [ ] ChunkCoord-based world (32x32m chunks, см. ADR-006)
   - [ ] Faction placement (procedural)
   - [ ] Economy initialization (supply/demand)
   - [ ] Procedural NPC population

2. **Configuration System:**
   - [ ] WorldConfig struct (26+ параметров)
   - [ ] CampaignConfig (per campaign type)
   - [ ] Seed encoding/decoding (VR-SEED-CAMPAIGN-HASH format)
   - [ ] Presets system (quick start templates)

3. **Freeplay Mode:**
   - [ ] No campaign objectives
   - [ ] All emergent systems active
   - [ ] Sandbox tools (exploration, trading)

**Checkpoint:**
- ✅ Можно создать custom seed с любыми параметрами
- ✅ World генерируется детерминистично (один seed = один мир)
- ✅ Freeplay mode работает (можно играть без campaign)

---

#### Phase 5.2: Campaign Framework (Mechanics) — 2-3 недели

**Milestone:** Три кампании механически работают (без hand-crafted content)

**Задачи:**

1. **Campaign State Machine:**
   - [ ] CampaignState enum (LastHope / BloodDebt / FinalDawn / Endgame / Freeplay)
   - [ ] Act tracking для кампаний
   - [ ] Objective system
   - [ ] Win condition evaluation

2. **Procedural Target Generation (Revenge Arc):**
   - [ ] RevengeTarget struct
   - [ ] Target chain generation (3-10 targets от FieldAgent до Leader)
   - [ ] Betrayal context generation (background × severity matrix)
   - [ ] Intel requirements system

3. **Timer Systems:**
   - [ ] Galactic Threat time limit (optional)
   - [ ] Final Dawn death timer (degradation, extensions)

4. **Quest System:**
   - [ ] ProceduralQuest struct
   - [ ] Quest templates (Escort, Eliminate, Trade, Investigate, Rescue, Defend)
   - [ ] Quest generation (world state-based)
   - [ ] Dynamic events (pirate raids, economic crisis, faction wars)

**Checkpoint:**
- ✅ Можно начать любую из трёх кампаний
- ✅ Revenge Arc генерирует chain of targets
- ✅ Final Dawn death timer работает (degradation, permadeath)
- ✅ Procedural quests генерируются бесконечно

---

#### Phase 5.3: Endgame Systems — 2-3 недели

**Milestone:** Post-campaign freeplay работает

**Задачи:**

1. **Emergent Systems:**
   - [ ] Faction war simulation (autonomous warfare)
   - [ ] Economic simulation (supply/demand, price volatility)
   - [ ] Reputation propagation (gossip system)
   - [ ] Dynamic events (random encounters)

2. **Faction Management (Blood Debt - Take Control):**
   - [ ] FactionLeadership struct
   - [ ] Daily income/expenses
   - [ ] Stability system (rebellions, coups, corruption)
   - [ ] Lieutenant management
   - [ ] Territory expansion

3. **Procedural Quest System:**
   - [ ] Radiant quests (см. procedural-narrative.md)
   - [ ] World state awareness (post-war vs civil war vs stable)
   - [ ] Reward scaling

**Checkpoint:**
- ✅ После завершения кампании открывается endgame freeplay
- ✅ Фракции автономно воюют за территории
- ✅ Faction management работает (Take Control path)
- ✅ Procedural quests бесконечны, разнообразны

---

#### Phase 5.4: Content & Polish (Story Mode) — 2-4 недели

**Milestone:** Три curated campaigns с hand-crafted content готовы

**Задачи:**

1. **Predefined Seeds:**
   - [ ] Seed "Invasion" (The Last Hope) — alien threat, wartime economy
   - [ ] Seed "Betrayal" (Blood Debt) — MegaCorp dominance, underworld
   - [ ] Seed "Twilight" (Final Dawn) — post-crisis, depression economy

2. **Hand-Crafted NPCs:**
   - [ ] Key NPCs для каждой кампании (faction leaders, targets, allies)
   - [ ] Unique dialogues
   - [ ] Personal quests

3. **Hand-Crafted Post-Game Questlines:**
   - [ ] The Last Hope: "The Aftermath", "War Crimes Tribunal", "New Threat" (5-10 missions total)
   - [ ] Blood Debt (Take Control): "The Usurper", "Old Debts", "Alliance or War"
   - [ ] Blood Debt (Walk Away): "Ghost of the Past", "The Aftermath", "New Purpose"

4. **Community Features:**
   - [ ] Seed sharing UI (import/export codes)
   - [ ] Workshop integration (optional)
   - [ ] Leaderboards (optional)

**Checkpoint:**
- ✅ Story Mode полностью играбелен (3 кампании начало-до-конца)
- ✅ Hand-crafted content добавляет narrative depth
- ✅ Seed sharing работает (можно делиться custom worlds)

---

**Deliverables (Фаза 5 полностью):**

**Core Systems:**
- `voidrun_simulation/src/campaign/` — state machine, objectives
- `voidrun_simulation/src/world/config.rs` — WorldConfig, seed system
- `voidrun_simulation/src/narrative/procedural.rs` — target/quest generation
- `voidrun_simulation/src/endgame/` — faction management, emergent systems

**Content:**
- `data/campaigns/` — predefined seeds, NPC definitions
- `data/quests/` — hand-crafted questlines
- `docs/design/` — campaign system design docs (✅ готовы)

**UI:**
- `voidrun_godot/src/ui/campaign_menu.rs` — campaign selection
- `voidrun_godot/src/ui/world_config.rs` — sandbox configuration UI
- `voidrun_godot/src/ui/seed_sharing.rs` — import/export

---

### Фаза 6: Space Flight & Combat

**Срок:** 3-4 недели
**Статус:** 🔜 После Фазы 5

- 6DOF полет
- Dogfight 1v1
- Transitions планета ↔ космос

---

### Фаза 7: Full Godot Integration

**Срок:** 2-3 недели
**Статус:** 🔜 После основных систем

- Custom bridge optimization
- Полноценные модели и анимации
- UI/UX polish

---

### Фаза 8: Content Expansion

**Срок:** Ongoing (постоянная разработка)
**Статус:** Параллельно с другими фазами

- 100+ items
- 50+ NPC archetypes
- 20+ ship types
- Procedural biomes/dungeons

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

---

## 🎯 Следующий шаг

**Текущий статус:** Phase 2 Domain Refactoring завершён ✅ (2025-10-26)

**Следующий приоритет: Chunk System & Procgen (6-10 дней)**

См. Фазу 1.5.5 выше для детального плана.

После procgen → Behaviour Tree для AI (когда будет полноценный мир для взаимодействия).

---

## 📋 Архив: Завершённые задачи

**✅ Melee Combat System (2-3 дня) — ЗАВЕРШЕНО**

Реализована полная система melee атак по образцу ranged combat:

**ECS Layer (Strategic):**
1. `MeleeAttackIntent` event — AI хочет атаковать
2. `ai_melee_attack_intent` система — генерирует intent когда:
   - AI в Combat state
   - Attacker cooldown готов
   - Target в радиусе melee (< 2м)
3. `MeleeAttackStarted` event — атака одобрена Godot

**Godot Layer (Tactical):**
1. `process_melee_attack_intents` — validate distance (Godot Transform)
2. `execute_melee_attacks` — trigger animation + enable hitbox
3. Melee weapon prefab (sword TSCN с Area3D hitbox)
4. `MeleeHit` event → `DamageDealt`

**Приоритет 2: Shield System (2-3 дня)**

После того как melee работает, добавить shields:

1. `Shield` component (energy, threshold, regen)
2. Modify damage systems:
   - Ranged damage → разряжает щит
   - Melee damage → игнорирует щит
3. Shield визуализация (bars + VFX)
4. Balance tests

**Итого:** ~5 дней до fully working combat prototype (melee + ranged + shields)

**Потом:** Player control или Chunk system (на выбор)
