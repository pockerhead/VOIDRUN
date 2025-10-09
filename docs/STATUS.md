# VOIDRUN — Текущее состояние проекта

**Дата:** 2025-01-09
**Фаза:** ✅ Фаза 1 завершена, → Фаза 1.5 (Presentation Layer)

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

### Цель: Presentation Layer Abstraction
**Зачем:** Отделить simulation от Godot, сделать независимым

### Задачи (3-5 дней):
1. Создать `PresentationClient` trait
2. Определить `PresentationEvent` enum
3. Event emission system в simulation
4. Refactor SimulationBridge → GodotPresentationClient
5. Создать HeadlessPresentationClient (no-op)
6. Update tests: использовать HeadlessClient

### После этого:
- Simulation = 100% независима от Godot
- Легко добавить другие рендеры (Bevy, web, etc)
- Headless CI tests без godot dependency
- Моддинг-friendly архитектура

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

### Presentation Layer (priority):
- [ ] Simulation зависит от Godot (tight coupling)
- [ ] Tests требуют прямого ECS доступа
- [ ] Нет абстракции для других рендеров

### Combat (polish later):
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

## 📝 Решения и Trade-offs

### Rapier роль:
- **Решение:** Rapier только для collision detection (weapon hits)
- **Движение:** Direct velocity integration (проще, детерминистичнее)
- **Почему:** KinematicPositionBased не двигается от velocity, нужен CharacterController

### Collision groups:
- **Actors:** `Group::NONE` — проходят друг через друга
- **Weapons:** коллайдят только с actors
- **Почему:** Упростило AI (не застревают), weapons всё равно детектят hits

### Godot vs Bevy:
- **Решение:** Остаться на Godot для визуализации
- **Почему:** Редактор + UI toolkit > +3x FPS (не нужен для systems RPG)
- **Архитектура:** "Ассеты на Godot, крутим-вертим на Rust"

### Netcode:
- **Решение:** Client-Server (authoritative) вместо P2P rollback
- **Postponed:** После save/load системы
- **Почему:** Single-player priority, MMORPG-style gameplay

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
  - [docs/architecture/physics-architecture.md](architecture/physics-architecture.md)
  - [docs/architecture/godot-rust-integration.md](architecture/godot-rust-integration.md)
- **Project Vision:** [docs/project-vision.md](project-vision.md)

---

**Следующая сессия:** Начать Фазу 1.5 (Presentation Layer Abstraction)
