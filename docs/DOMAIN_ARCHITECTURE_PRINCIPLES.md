# Domain Architecture Principles - VOIDRUN

**Дата создания:** 2025-01-26
**Статус:** ✅ ACTIVE (применяется в продакшене)
**Источник:** Domain Refactoring Phase 1-2 (voidrun_simulation + voidrun_godot)

Этот документ содержит ключевые архитектурные принципы и решения, примененные при domain-driven refactoring. **Используй эти принципы для всех будущих архитектурных решений.**

---

## 🎯 Золотое правило: Domain-Driven Organization

### Принцип
Код организуется по **business domains**, НЕ по техническим слоям.

### ✅ ПРАВИЛЬНО (Domain-driven)
```
src/
├── combat/         # Business domain (melee + ranged + AI)
├── navigation/     # Business domain (pathfinding + avoidance)
├── player_shooting/# Business domain (ADS + hip fire)
└── shared/         # Cross-cutting concerns
```

### ❌ НЕПРАВИЛЬНО (Layer-driven)
```
src/
├── systems/        # Technical layer
├── components/     # Technical layer
├── events/         # Technical layer
└── resources/      # Technical layer
```

### Почему
- **Business понятность:** Код читается как requirements doc
- **Cohesion:** Связанный код находится рядом
- **Changes localization:** Изменение feature = изменение 1 domain
- **Separation:** Domain logic ≠ infrastructure (Clean Architecture)

---

## 📏 File Size Limits (ЖЁСТКОЕ ПРАВИЛО)

### Лимиты
- **Warning:** >750 строк — подумай о split
- **Hard limit:** >950 строк — НЕПРИЕМЛЕМО, обязательный split

### Действия при >750 строк
1. **СТОП** — остановить разработку
2. **Обсуждение** с user — архитектурные trade-offs
3. **Логические блоки** — определить для split
4. **Split pattern:** Multiple `impl` blocks (как Swift extensions), НЕ standalone функции

### Пример
```rust
// ✅ ПРАВИЛЬНО: Logical split через impl blocks
// scene.rs
impl SimulationBridge {
    pub(super) fn create_camera(&mut self) { ... }
}

// mod.rs
self.create_camera();  // Вызов как метод

// ❌ НЕПРАВИЛЬНО: Standalone функция с параметром
// scene.rs
pub fn create_camera(parent: &mut Gd<Node3D>) { ... }

// mod.rs
create_camera(self.base_mut());  // Громоздко
```

### Почему
- >750 строк — сложно держать в голове
- >950 строк — code review nightmare
- ECS/event-driven код НЕ должен иметь монстр-файлы
- Если файл растёт — значит нарушена модульность

---

## 🏗️ Domain Module Structure

### Стандартная структура
```rust
src/domain_name/
├── mod.rs              // Re-exports + domain resources
├── components.rs       // ECS components (опционально)
├── systems.rs          // ECS systems (опционально)
└── events.rs           // Domain events (опционально)
```

### Когда split внутри domain
**Правило:** Split если domain >750 строк ИЛИ несколько logical concerns.

```rust
src/combat/             // Unified domain (2282 строки)
├── mod.rs              // Re-exports
├── melee/              // Subdomain (467 строк)
│   └── mod.rs
├── ai_melee/           // Subdomain (913 строк, split на 4 файла)
│   ├── mod.rs
│   ├── validation.rs
│   ├── decision.rs
│   └── evaluation.rs
└── ranged/             // Subdomain (902 строки, split на 4 файла)
    ├── mod.rs
    ├── targeting.rs
    ├── ranged_attack.rs
    └── projectile.rs
```

### Re-export pattern
```rust
// domain/mod.rs
pub mod subdomain_a;
pub mod subdomain_b;

// Re-export все public items
pub use subdomain_a::*;
pub use subdomain_b::*;
```

**Преимущество:** Простота использования (`use crate::domain::*;`)

---

## 🔑 Domain Decision Framework

### Вопросы перед созданием domain

1. **Концептуальная связность**
   - Эти файлы **conceptually related**? (business concept OR technical concern?)
   - Они решают **одну** бизнес-задачу?

2. **Cohesion (связность)**
   - Они изменяются **together**? (same причина для changes?)
   - Они зависят от **одинаковых** external modules?

3. **Размер**
   - Общий размер domain **reasonable**? (<1500 строк total)
   - Можно ли разумно split если превышает?

4. **Infrastructure vs Domain**
   - Это domain logic (business rules) ИЛИ infrastructure (technical detail)?
   - Можно ли переиспользовать в других domains?

### Пример анализа: shooting domain

**Вопрос:** Объединить `shooting` с `input` domain?

**Анализ:**
- **Концепция:** Input = "что игрок нажал?" (device layer), Shooting = "как weapon движется?" (gameplay layer) → **Разные**
- **Cohesion:** Input handling ≠ Transform calculations → **Низкая**
- **Размер:** 677 + 384 = 1061 строка → **Нарушение лимита**
- **Infrastructure vs Domain:** Input = infrastructure, Shooting = domain logic → **Разделяй**

**Решение:** Отдельный `player_shooting/` domain ✅

---

## 🎨 Architectural Patterns

### Pattern 1: Separation of Concerns

**Принцип:** Domain logic ≠ Infrastructure.

**Примеры:**
```rust
// ✅ ПРАВИЛЬНО: Separated
input/              // Infrastructure (keyboard/mouse polling)
player_shooting/    // Domain logic (weapon positioning math)

// ❌ НЕПРАВИЛЬНО: Mixed
player_interaction/ // Input handling + shooting logic + abilities
```

### Pattern 2: Unified Domain для Related Concerns

**Принцип:** Если concerns **conceptually related** → unified domain с subdomains.

**Пример:**
```rust
// ✅ ПРАВИЛЬНО: Unified
combat/
├── melee/      // Melee execution
├── ai_melee/   // AI combat decisions
└── ranged/     // Ranged combat

// ❌ НЕПРАВИЛЬНО: Scattered
src/
├── melee_system/
├── ai_melee_combat_decision/
└── weapon_system/  // Actually ranged combat (confusing naming!)
```

**Обоснование:** Melee + Ranged + AI decisions = **combat mechanics** (shared context)

### Pattern 3: Shared для Cross-Cutting Concerns

**Принцип:** Utilities используемые **multiple domains** → `shared/`.

**Что идёт в shared:**
- Resources: VisualRegistry, AttachmentRegistry, SceneRoot
- Utilities: actor_utils (mutual facing), los_helpers (raycast)
- Constants: collision layers/masks
- Common types: StrategicPosition, PrefabPath

**Что НЕ идёт в shared:**
- Domain-specific logic (должно быть в domain)
- Business rules (должны быть в domain)

**Risk:** Shared может разрастись → **периодический review** (каждые 2 недели)

### Pattern 4: Backward Compatibility через Re-exports

**Принцип:** Старый API продолжает работать после рефакторинга.

```rust
// components/mod.rs (old API location)
pub use crate::actor::*;
pub use crate::movement::*;
pub use crate::shooting::*;

// Старый код продолжает работать:
use voidrun_simulation::components::{Actor, Health};

// Новый код может использовать прямые imports:
use voidrun_simulation::actor::{Actor, Health};
```

**Преимущество:** Постепенная миграция без breaking changes

---

## ⚙️ Technical Practices

### Practice 1: Incremental Compilation Checks

**Правило:** `cargo check` после **КАЖДОГО** domain migration.

```bash
# 1. Создать domain structure
mkdir -p src/new_domain
cp src/old.rs src/new_domain/file.rs

# 2. Update imports
sed -i 's/old/new/g' ...

# 3. CHECK COMPILATION (CRITICAL!)
cargo check --package <package>

# 4. ONLY after success → delete old
rm src/old.rs
```

**Почему:** Раннее обнаружение ошибок, меньше debugging времени

### Practice 2: Batch Operations для Imports

**Принцип:** sed + find >> множество Edit calls.

```bash
# 1. Find all usage (verify scope)
grep -r "old_import" --include="*.rs"

# 2. Count occurrences
grep -r "old_import" --include="*.rs" | wc -l

# 3. Batch replace
find . -name "*.rs" -exec sed -i 's/old_import/new_import/g' {} +

# 4. Verify (should be empty)
grep -r "old_import" --include="*.rs"
```

**Выигрыш:** 0.3 сек vs 1-2 мин для manual edits

**Trade-off:** Нужна уверенность в pattern (grep проверка перед sed)

### Practice 3: Golden Path (let-else)

**Принцип:** Линейный код без вложенности.

```rust
// ✅ ПРАВИЛЬНО: Linear flow
let Some(value) = optional else { return; };
let Ok(result) = fallible else {
    log_error("Failed");
    return;
};
do_something(value, result);

// ❌ НЕПРАВИЛЬНО: Nested hell
if let Some(value) = optional {
    if let Ok(result) = fallible {
        do_something(value, result);
    } else {
        log_error("Failed");
    }
}
```

**Применение:** Везде в voidrun_godot (2+ уровня вложенности → рефактори)

### Practice 4: Domain Cohesion Analysis перед Merge

**Процесс:**
1. Определить **варианты** (A, B, иногда C)
2. **Pros/Cons** для каждого
3. **Best practices** ссылки (Clean Architecture, DDD, SOLID)
4. **Обоснование** выбора

**Пример:** `shooting` merge с `input`?
- Вариант A: Отдельный player_shooting ✅
- Вариант B: Merge с input ❌
- Обоснование: Infrastructure ≠ domain logic, size limit violation

---

## 📋 Architecture Decision Checklist

### Перед созданием нового domain:

- [ ] Определён business concept ИЛИ technical concern?
- [ ] Размер domain <1500 строк (или reasonable split plan)?
- [ ] Cohesion высокая? (files change together)
- [ ] НЕ смешивается domain logic с infrastructure?
- [ ] Проверены варианты (merge vs separate vs shared)?
- [ ] Trade-offs analysis выполнен?
- [ ] Best practices references найдены?

### Перед split файла:

- [ ] Файл >750 строк?
- [ ] Логические блоки определены?
- [ ] Split pattern выбран? (impl blocks OR subdomains)
- [ ] Re-exports structure продумана?
- [ ] Imports migration plan готов?

### После рефакторинга:

- [ ] `cargo check` успешен?
- [ ] Старые файлы удалены?
- [ ] Backward compatibility сохранена? (re-exports)
- [ ] Documentation обновлена?
- [ ] Trade-offs задокументированы?

---

## 🎓 Lessons Learned (ключевые инсайты)

### 1. Infrastructure vs Domain Logic — КЛЮЧЕВОЕ разделение

**Insight:** НЕ смешивай technical concerns с business logic.

**Примеры разделения:**
- `input/` (keyboard polling) ≠ `player_shooting/` (weapon math)
- `shared/collision.rs` (constants) ≠ `combat/` (damage execution)
- `navigation/avoidance.rs` (Godot wrapper) ≠ `combat/ai_melee/` (AI decisions)

**Применение:** Всегда спрашивай — это "как работает система" (infrastructure) OR "что делает игра" (domain)?

### 2. Размер файла — прямой индикатор модульности

**Insight:** Если файл растёт >750 строк — значит **нарушена модульность**.

**Причины:**
- Смешаны несколько concerns
- Недостаточно abstraction layers
- Monolithic thinking

**Решение:** Split по logical blocks, НЕ arbitrary line count

### 3. Batch operations — огромный time saver

**Insight:** sed + find быстрее в **10-30 раз** vs множество Edit calls.

**Применение:** Imports migration, renaming, pattern replacement

**Риск:** Нужна careful verification (grep before/after)

### 4. Trade-off analysis — избегай интуитивных решений

**Insight:** Осознанные решения >> gut feeling.

**Процесс:**
1. Варианты (A, B, C)
2. Pros/Cons
3. Best practices
4. Обоснование

**Результат:** Архитектурные решения которые можно объяснить

### 5. Backward compatibility — критична для больших refactorings

**Insight:** Re-exports позволяют **постепенную** миграцию.

**Преимущество:** Нет big bang refactor (меньше риск)

---

## 🚀 Best Practices References

### Clean Architecture (Robert Martin)
- **Key:** Domain logic независима от infrastructure
- **Application:** Separation of concerns (domain vs infrastructure layers)
- **Книга:** "Clean Architecture: A Craftsman's Guide to Software Structure"

### Domain-Driven Design (Eric Evans)
- **Key:** Modules отражают business concepts
- **Application:** Domain organization (combat, navigation, shooting)
- **Книга:** "Domain-Driven Design: Tackling Complexity in the Heart of Software"

### SOLID Principles
- **SRP:** Single Responsibility (один модуль = одна причина изменений)
- **OCP:** Open/Closed (domain structure extensible без модификаций)
- **Application:** player_shooting ≠ input (SRP violation если merge)

### Bevy ECS Best Practices
- **Key:** Systems группируются по domain responsibility
- **Application:** combat/ranged/ — все ranged systems вместе
- **Источник:** Bevy documentation + community patterns

### Rust API Guidelines
- **Key:** Re-exports для ergonomic API
- **Application:** mod.rs re-export pattern
- **Источник:** https://rust-lang.github.io/api-guidelines/

---

## 📝 Quick Reference Card

### Domain Creation Decision Tree

```
Новый код для feature X?
│
├─ Infrastructure concern? (polling, wrappers, constants)
│  └─ → shared/ OR new infrastructure module
│
├─ Domain logic? (business rules, gameplay)
│  │
│  ├─ Связан с existing domain? (combat, navigation, etc)
│  │  └─ → добавить в existing domain (если <1500 строк total)
│  │
│  └─ Новый business concept?
│     └─ → создать new domain
│
└─ Size >750 строк?
   └─ → Split на logical subdomains
```

### Import Migration Process

```bash
# 1. Grep verification
grep -r "old::path" --include="*.rs" | wc -l

# 2. Batch replace
find . -name "*.rs" -exec sed -i 's/old::path/new::path/g' {} +

# 3. Verify replacement
grep -r "old::path" --include="*.rs"  # Should be empty

# 4. Compilation check
cargo check --package <package>
```

### File Split Pattern

```rust
// Original: monolith.rs (950 строк)

// Split option 1: Logical subdomains
domain/
├── subdomain_a/
└── subdomain_b/

// Split option 2: Multiple impl blocks
domain/
├── mod.rs        # Main struct + core logic
├── feature_a.rs  # impl MyStruct (feature A methods)
└── feature_b.rs  # impl MyStruct (feature B methods)
```

---

## ⚠️ Common Pitfalls (избегай)

### Pitfall 1: Technical Layer Organization

**❌ ПЛОХО:**
```
src/systems/    # All systems together
src/components/ # All components together
```

**Проблема:** Нет domain понятности, changes scatter across layers

**✅ ХОРОШО:**
```
src/combat/        # Combat domain (components + systems + events)
src/navigation/    # Navigation domain
```

### Pitfall 2: Mixed Concerns в одном Domain

**❌ ПЛОХО:**
```
src/player_interaction/  # Input + shooting + abilities + inventory
```

**Проблема:** Low cohesion, multiple reasons to change, >2000 строк

**✅ ХОРОШО:**
```
src/input/           # Input handling (infrastructure)
src/player_shooting/ # Shooting mechanics (domain)
src/abilities/       # Abilities system (domain)
src/inventory/       # Inventory management (domain)
```

### Pitfall 3: Premature Abstraction

**❌ ПЛОХО:**
```rust
// Создание abstraction layer "на будущее"
pub trait GenericShootingSystem { ... }
pub struct ShootingFacade { ... }
pub struct ShootingAdapter { ... }
```

**Проблема:** YAGNI violation, overengineering

**✅ ХОРОШО:**
```rust
// Простые re-exports в mod.rs
pub use ads::*;
pub use hip_fire::*;
```

### Pitfall 4: Ignoring Size Limits

**❌ ПЛОХО:**
```rust
// monolith_system.rs (1500 строк)
// "Разделю потом, когда будет время"
```

**Проблема:** Technical debt растёт, рефакторинг становится сложнее

**✅ ХОРОШО:**
```rust
// При >750 строк — СТОП и split
domain/
├── subdomain_a.rs  (400 строк)
└── subdomain_b.rs  (350 строк)
```

---

**Версия:** 1.0
**Дата:** 2025-01-26
**Статус:** ACTIVE (применяется во всех новых features)
**Источник:** DOMAIN_REFACTORING_PLAN.md (Phase 1-2 lessons)
