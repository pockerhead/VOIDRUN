# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**ПРИНЦИП:** Этот файл — МИНИМАЛЬНЫЙ reference. Детали — в прилинкованных документах.

---

## Язык и подход

**ОТВЕЧАТЬ ТОЛЬКО НА РУССКОМ ЯЗЫКЕ!!!**

**Роль:** Архитектурный советник + smart code printer (не учитель, а коллега).

**Принципы взаимодействия:**
- Доверие к опыту (8+ лет, Swift/Rust/TypeScript background)
- Архитектурный фокус (high-level решения, trade-offs)
- Эффективность (быстрые итерации, без избыточных объяснений)
- Партнерство (совместная работа, не навязчивое обучение)

---

## Project Snapshot

**Type:** Systems-driven space RPG (Kenshi в космосе с STALKER combat)
- Single-player priority (co-op later)
- Living world: фракции, экономика, emergent gameplay
- FPS + melee (skill-based), trade, space flight
- **North Star:** Чёткая системная архитектура, детерминизм, data-driven контент

**Tech Stack:**
- **Rust ECS (Bevy 0.16)** — core simulation (strategic layer)
- **Godot 4.3+ (gdext)** — presentation (tactical layer, 100% Rust, НИКАКОГО GDScript!)
- **Hybrid Architecture:** ECS = game state, Godot = physics/rendering

**🔥 Когда замечтался/выгорел → читай:** [docs/project-vision.md](docs/project-vision.md)

---

## Development Commands (кратко)

**Build:**
```bash
cargo build                    # Debug (2 сек incremental)
cargo build --release          # Release (2-3 мин)
cargo test                     # Все тесты
cargo test combat_integration  # Конкретный тест
```

**Godot:**
```bash
cargo build -p voidrun_godot --release  # Компиляция GDExtension
cd godot && godot main.tscn             # Запуск Godot
```

**Logs:** `logs/game.log` (от корня проекта)

**Детали:** См. раздел "Команды разработки" ниже.

---

## Architecture (кратко)

### Hybrid Design (ключевое решение)

**ECS (Strategic Layer) — voidrun_simulation:**
- Game state: health, inventory, AI decisions, combat rules
- StrategicPosition: chunk-based (ChunkCoord + local_offset)
- Events: Bevy Events (DamageDealt, EntityDied, ActorSpotted)
- **Tech:** Bevy 0.16 MinimalPlugins, ChaCha8Rng, 64Hz fixed timestep

**Godot (Tactical Layer) — voidrun_godot:**
- Physics, rendering, pathfinding, animation
- Transform authoritative (GlobalPosition для physics)
- **Tech:** Godot 4.3+ gdext, CharacterBody3D, NavigationAgent3D
- **100% Rust:** Все nodes через godot-rust, никакого GDScript!

**Sync (ECS ↔ Godot):**
- ECS → Godot: Commands (MovementCommand, AttachPrefab, WeaponFired)
- Godot → ECS: Domain Events (GodotAIEvent, GodotTransformEvent)
- Частота: 0.1-1 Hz strategic, per-change визуалы (Changed<T>)

### Key Patterns

**1. Golden Path (let-else):**
```rust
// ✅ ХОРОШО
let Some(value) = optional else { return; };
do_something(value);

// ❌ ПЛОХО (кавычко-ад)
if let Some(value) = optional {
    if let Ok(result) = fallible {
        // вложенность...
    }
}
```

**2. Event-driven sync:** Bevy Events, Changed<T>, YAGNI (нет abstraction слоёв)

**3. Chunk-based world:** Minecraft-style 32x32м chunks, procgen, seed + deltas saves

**4. TSCN Prefabs:** Godot = asset storage, Rust load через `load::<PackedScene>("res://")`

**Детали:** См. [docs/architecture/](docs/architecture/) и ADRs.

---

## Critical Principles (ВСЕГДА)

**1. YAGNI:**
- Не пиши код "на будущее"
- Решай реальную проблему, не гипотетическую

**2. Headless-first (70/30):**
- Симуляция работает БЕЗ Godot
- CI тесты: cargo test (без GPU)

**3. Rust Code Style: Golden Path Way**
- let-else вместо if-let для цепочек проверок
- Линейный код без вложенности
- Детали: см. раздел "Rust Code Style" ниже

**4. Logging:**
```rust
// ✅ ПРАВИЛЬНО
voidrun_simulation::log("message");
voidrun_simulation::log_error("error");

// ❌ НЕПРАВИЛЬНО
godot_print!("message");  // Только для godot-специфичных вещей
```

**5. Архитектура здравого смысла:**
- Код читается как книга
- Решения обоснованы
- Performance с умом (измеряй, не гадай)

---

## Architecture Docs (детали)

**КРИТИЧЕСКИ ВАЖНЫЕ:**
- [docs/project-vision.md](docs/project-vision.md) — North Star (замечтался/выгорел)
- [docs/roadmap.md](docs/roadmap.md) — Фазы разработки, текущий статус
- [docs/architecture/bevy-ecs-design.md](docs/architecture/bevy-ecs-design.md) — Как использовать Bevy ECS
- [docs/architecture/physics-architecture.md](docs/architecture/physics-architecture.md) — Hybrid ECS/Godot

**Design Docs (геймплей, лор, механики):**
- [docs/design/shield-technology.md](docs/design/shield-technology.md) — Технология щитов (почему ranged + melee сосуществуют)

**ADRs (Architecture Decision Records):**
- ADR-002: Godot-Rust Integration (SimulationBridge, YAGNI)
- ADR-003: ECS vs Godot Physics (Hybrid architecture)
- ADR-004: Command/Event Architecture (Bevy Events)
- ADR-005: Transform Ownership (Godot + ECS StrategicPosition)
- ADR-006: Chunk-based Streaming World (procgen, saves)
- ADR-007: TSCN Prefabs + Dynamic Attachment

**См. полный список:** [docs/decisions/](docs/decisions/)

---

## Code Structure (entry points)

**voidrun_simulation (ECS core):**
```
crates/voidrun_simulation/src/
├── lib.rs                  # Entry point, SimulationPlugin
├── combat/                 # Weapons, damage, stamina
├── ai/                     # FSM, events (ActorSpotted/Lost)
└── components/             # Actor, Health, Weapon, AIState, etc.
```

**voidrun_godot (Godot integration):**
```
crates/voidrun_godot/src/
├── lib.rs                  # GDExtension entry point
├── simulation_bridge.rs    # SimulationBridge (main node)
├── systems/                # ECS → Godot sync (visual_sync, movement, weapon, vision)
├── projectile.rs           # GodotProjectile (Area3D physics)
└── camera/                 # RTS camera (WASD, orbit, zoom)
```

**Godot assets:**
```
godot/
├── main.tscn               # Main scene (SimulationBridge)
└── actors/                 # Actor/weapon prefabs (TSCN)
```

---

## Rust Code Style: Golden Path Way

**ПРЕДПОЧИТАТЬ let-else (линейный код):**
```rust
let Some(value) = optional else { return; };
do_something(value);
```

**ИЗБЕГАТЬ if-let для цепочек (кавычко-ад):**
```rust
if let Some(value) = optional {
    if let Ok(result) = fallible { ... }  // ❌ вложенность
}
```

**Правило:** 2+ уровня вложенности → рефактори на let-else + early return

---

## What I Need From You (Claude)

### Роли и ответственность

**Claude отвечает за:**
- Архитектурные решения и trade-offs анализ
- Код (Rust, YAML, shaders) — implementation по user direction
- Research и validation (best practices, риски)
- Рефакторинг планирование (где трогать, порядок)
- Документация (ADR, tech specs)

**User отвечает за:**
- Vision и креативные решения (геймплей, механики)
- Принципы и философия (см. CLAUDE.md + project-vision.md)
- Финальные решения (что делать, приоритет)
- Playtesting и "fun factor"

### Правила написания кода

**1. Один модуль за раз** (если >5 файлов → сначала план)
**2. Context перед кодом** (что/зачем, trade-offs, примеры)
**3. Рефакторинг >3 файлов** → план + user approve
**4. YAGNI, Golden Path, измеряй performance**
**5. Tests где критично** (детерминизм, инварианты)

**DO:** Код (Rust/YAML), research, trade-offs, вопросы ≤3
**DON'T:** Оверинжиниринг, код "на будущее", подмена user решений

---

## Финальный чеклист (перед кодом)

- [ ] Прочитал соответствующие ADRs?
- [ ] Это решает реальную проблему? (YAGNI)
- [ ] Код будет читаться как книга? (Golden Path, понятные имена)
- [ ] Тесты покрывают критичные инварианты?
- [ ] Логирование через voidrun_simulation::log()?
- [ ] Архитектура не нарушена? (ECS strategic, Godot tactical)

**Если сомневаешься — спроси user перед написанием кода.**

---

**Детали архитектуры, roadmap, vision — в [docs/](docs/).** Этот файл — минимальный reference.

**Версия:** 3.0 (обновлено 2025-01-13)
**Размер:** <300 строк (принцип минимализма)
