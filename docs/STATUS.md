# VOIDRUN — Текущее состояние проекта

**Дата:** 2025-01-10
**Фаза:** ✅ Фаза 1 завершена, → Фаза 1.5 (Combat Mechanics + Player Control)

---

## ✅ Что работает прямо сейчас

### Combat System
- ✅ 2 NPC дерутся друг с другом
- ✅ AI: target detection, approach, attack, retreat
- ✅ Weapon hitbox: меч-капсула 1.5m, swing animation
- ✅ Damage system: base damage × stamina multiplier
- ✅ Stamina: attack cost 30, regen 10/sec, exhaustion
- ✅ Детерминизм: 1000 ticks, 3 runs → identical results

### Visualization (Godot + Rust)
- ✅ 100% Rust visuals (no GDScript)
- ✅ Health bar (красная, над головой)
- ✅ Stamina bar (зелёная, под health)
- ✅ AI state label (желтая, показывает Idle/Aggro/Attack)
- ✅ Weapon mesh: diagonal pose, swing animation sync
- ✅ Hit particles: красные сферы при damage
- ✅ RTS camera: WASD pan, RMB orbit, scroll zoom

### Tests
- ✅ 28 unit tests — all passing
- ✅ 3 integration tests (combat_integration.rs)
- ✅ 2 determinism tests
- ✅ Godot runtime: работает, визуалы видны

---

## 🏗️ Архитектура

### Текущая структура:
```
Rust Simulation (voidrun_simulation)
├── ECS (Bevy 0.16)
│   ├── Components: Actor, Health, Stamina, Weapon
│   ├── Systems: AI FSM, combat, physics
│   └── Events: AttackStarted, DamageDealt, EntityDied
│
└── Physics (bevy_rapier3d 0.31)
    ├── Movement: direct velocity integration
    ├── Collisions: weapon hits only (actors pass through)
    └── Determinism: 64Hz fixed timestep

Godot Visualization (voidrun_godot)
└── SimulationBridge
    ├── Creates 3D scene programmatically (Rust)
    ├── Syncs: transforms, health, stamina, AI state
    └── Effects: particles, weapon swing animation
```

### Принципы:
- **Rust = simulation** (systems, logic, determinism)
- **Godot = presentation** (visuals, UI, assets)
- **Headless-first** (tests без Godot)

---

## 🎯 Следующие шаги (Фаза 1.5)

### Цель: Combat Mechanics + Player Control
**Зачем:** Сделать игру играбельной, проверить combat feel

### Задачи (5-8 дней):

**Player Control (1-2 дня):**
- WASD movement
- Mouse attack (LMB → swing)
- Camera follow (3rd person)
- Health/Stamina HUD

**Melee Polish (1 день):**
- Parry timing (200ms window)
- Block action (RMB hold)
- Dodge roll (spacebar, i-frames)

**Ranged Combat (2-3 дня):**
- Projectile physics (gravity, arc)
- Bow/crossbow weapon
- Ranged AI behavior
- Hit detection

**AI Upgrade (1-2 дня):**
- Pathfinding (NavigationAgent3D)
- Ranged tactics (keep distance)
- Dodge projectiles

### После этого:
- Playable prototype (1 player vs NPCs)
- Combat feel проверен
- Foundation для inventory/loot
- Economy можно делать параллельно

---

## 📊 Метрики

### Code Stats:
- **voidrun_simulation:** ~2000 lines Rust
  - physics/movement.rs: 264 lines
  - combat/weapon.rs: 340 lines
  - ai/simple_fsm.rs: 350 lines
  - combat/damage.rs: 180 lines
  - combat/stamina.rs: 150 lines

- **voidrun_godot:** ~400 lines Rust
  - simulation_bridge.rs: 400 lines

- **Tests:** 5 test files, 33 tests total

### Build Times:
- Incremental: ~2 sec
- Full rebuild: ~40 sec
- Tests run: ~0.3 sec

### Performance:
- 64Hz fixed update (Rust simulation)
- Godot render: ~60 FPS
- 2 NPC fight: no performance issues

---

## 🛠️ Tech Stack

### Core:
- **Bevy 0.16** — ECS framework
- **bevy_rapier3d 0.31** — physics (collision detection only)
- **Godot 4.3+** — visualization, UI
- **godot-rust** — GDExtension bindings

### Rust Crates:
- `rand_chacha` — deterministic RNG
- `serde` — serialization (future save/load)

### Конфигурация:
- `default-features = false` для rapier (headless-friendly)
- `-C target-feature=-fma` для детерминизма
- `enhanced-determinism` feature enabled

---

## 🔍 Известные Issues / TODO

### Player Control (priority):
- [ ] Нет player entity (только NPC vs NPC)
- [ ] Нет input handling
- [ ] Camera RTS mode (нужен follow mode)
- [ ] Нет HUD для player

### Combat (в работе):
- [ ] Parry window не реализован (200ms timing)
- [ ] Block action отсутствует
- [ ] Dodge отсутствует
- [ ] Weapon trail particles (visual только)

### AI (future):
- [ ] Pathfinding (сейчас прямая линия к цели)
- [ ] Group tactics
- [ ] Cover usage

### Визуализация (future):
- [ ] Skeletal animations (сейчас capsule + swing rotation)
- [ ] Proper 3D models
- [ ] Sound effects
- [ ] UI polish (inventory, dialogue)

---

## 📝 Архитектурные решения (2025-01-10)

### ⚠️ КЛЮЧЕВОЕ РЕШЕНИЕ: Hybrid Architecture

**Дата:** 2025-01-10
**См. полное обоснование:** [ADR-003: ECS vs Godot Physics Ownership](decisions/ADR-003-ecs-vs-godot-physics-ownership.md)

**Суть решения:**
```
ECS (Strategic Layer)        Godot (Tactical Layer)
━━━━━━━━━━━━━━━━━━━━━━      ━━━━━━━━━━━━━━━━━━━━━━
✅ Game state (health, AI)   ✅ Transform (authoritative)
✅ Combat rules (damage)     ✅ Physics (CharacterBody3D)
✅ Economy, factions         ✅ Animations, hitboxes
✅ Strategic position        ✅ Pathfinding (NavAgent)
        ↓ commands ↑ events
```

**Ключевые изменения:**
- ❌ Rapier больше НЕ используется для movement (опционален)
- ✅ Godot Physics authoritative для всего physics
- ✅ ECS = brain (decisions), Godot = body (execution)

**Почему:**
- Single-player priority → детерминизм не критичен
- Client-Server netcode (не P2P) → не требует bit-perfect physics
- Godot features (NavigationAgent3D, AnimationTree) → меньше кода
- Фокус на systems (economy, AI) → точная физика не критична

---

### Презентационный слой (ADR-002):
- **Решение:** SimulationBridge без PresentationClient abstraction
- **Почему:** YAGNI — Godot работает отлично, смена движка = риск <5%
- **Assets:** Godot prefabs + Rust load через `load::<T>("res://")`

### Netcode (будущее):
- **Решение:** Client-Server (authoritative), не P2P rollback
- **Postponed:** После Combat Mechanics + Player Control
- **Почему:** Single-player priority, MMORPG-style gameplay

### Rapier роль (УСТАРЕЛО):
- ~~**Решение:** Rapier только для collision detection (weapon hits)~~
- **Новое:** Godot Physics для всего, Rapier опционален

---

## 🎮 Как запустить

### Тесты (headless):
```bash
cargo test -p voidrun_simulation
```

### Godot visualization:
```bash
# Build Rust library
cargo build --release -p voidrun_godot

# Run Godot project
cd godot
godot4 --path .
```

---

## 📚 Документация

- **Roadmap:** [docs/roadmap.md](roadmap.md)
- **Architecture:**
  - [docs/architecture/bevy-ecs-design.md](architecture/bevy-ecs-design.md)
  - [docs/architecture/physics-architecture.md](architecture/physics-architecture.md) ⚠️ v3.0 (Hybrid)
  - [docs/architecture/godot-rust-integration.md](architecture/godot-rust-integration.md)
  - [docs/architecture/presentation-layer-abstraction.md](architecture/presentation-layer-abstraction.md) ⏸️ (POSTPONED)
- **Decisions (ADRs):**
  - [ADR-002: Godot-Rust Integration Pattern](decisions/ADR-002-godot-rust-integration-pattern.md)
  - [ADR-003: ECS vs Godot Physics Ownership](decisions/ADR-003-ecs-vs-godot-physics-ownership.md) ⚠️ **КЛЮЧЕВОЕ**
- **Project Vision:** [docs/project-vision.md](project-vision.md)

---

**Следующая сессия:** Начать Фазу 1.5 (Combat Mechanics + Player Control)

---

## 🚫 Отложенные решения

### Presentation Layer Abstraction (POSTPONED)
- **Статус:** Отложено до после Vertical Slice
- **Причина:** YAGNI — решает проблему которой нет
- **Godot работает:** SimulationBridge = правильная архитектура
- **Детали:** См. [ADR-002](decisions/ADR-002-godot-rust-integration-pattern.md)

### Детерминистичная физика (NOT NEEDED)
- **Статус:** Не требуется для Hybrid Architecture
- **Причина:** Single-player priority, Client-Server netcode (не P2P rollback)
- **Fixed-point math:** Не нужен (f32/f64 достаточно)
- **Rapier determinism:** Проблема решена отказом от Rapier для movement
