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

## 🚧 Фаза 1: Physics + Combat Core (В РАБОТЕ)

**Срок:** 2-3 недели
**Статус:** 🔜 Ready to start

### Milestone цель:
**2 NPC дерутся headless 1000 тиков без крашей, детерминистично**

### Неделя 1-2: Physics Foundation
**Задачи:**
- [ ] Добавить bevy_rapier3d 0.31 (раскомментировать в Cargo.toml)
- [ ] Базовые компоненты: Position, Velocity, Health, Stamina
- [ ] Kinematic контроллер (WASD movement, gravity)
- [ ] Capsule коллизия для NPC
- [ ] Property-тесты: no NaN, velocity bounds, stamina [0,100]

**Пятница 1:** Debug render в Bevy
- Кубик двигается WASD
- Capsule коллизия видна (gizmos)
- Stamina bar над головой (text label)

### Неделя 3: Combat System
**Задачи:**
- [ ] Hitbox система: AttackHitbox компонент (sphere/capsule)
- [ ] Attack system: swing animation timing → spawn hitbox → check overlaps
- [ ] Damage calculation: base damage × stamina multiplier
- [ ] Stamina system: attack costs 30%, block 20%, regen 10%/sec
- [ ] Parry window: 200ms перед ударом врага
- [ ] Simple AI: FSM (Idle → Aggro → Approach → Attack → Retreat)

**Пятница 3:** Combat debug визуал
- 2 NPC дерутся
- Hitbox'ы атак видны (красные сферы)
- Stamina bars обновляются
- Проверка timing: чувствуется ли parry window

### Checkpoint Фазы 1:
- ✅ Headless тест: `cargo test combat_stress_test` (2 NPC, 1000 тиков)
- ✅ Property-тест: health/stamina инварианты
- ✅ Детерминизм: 3 прогона с seed=42 → идентичные snapshots
- ✅ Debug визуал показал: combat timing ощущается нормально

### Deliverables:
- `voidrun_simulation/src/physics/` — модуль с контроллером
- `voidrun_simulation/src/combat/` — hitbox, damage, stamina системы
- `voidrun_simulation/src/ai/simple_fsm.rs` — базовый AI
- `tests/combat_determinism.rs` — стресс-тесты

---

## 📋 Фаза 2: Rollback Netcode (PLANNING)

**Срок:** 2-3 недели
**Статус:** 🔜 После Фазы 1

### Milestone цель:
**2 клиента дерутся по сети с 100ms latency, rollback работает**

### Задачи:
- [ ] GGRS интеграция (P2P rollback netcode)
- [ ] Snapshot/Restore через `bevy_save` или custom
- [ ] Input prediction и reconciliation
- [ ] 2 headless клиента по UDP
- [ ] Latency simulation для тестов (50ms, 100ms, 150ms)
- [ ] Property-тест: rollback не ломает детерминизм

**Пятница debug визуал:**
- 2 окна Bevy рядом
- Видны rollbacks (мигание/ghosting?)
- Проверка: играбельно ли при 100ms?

### Checkpoint:
- ✅ 100ms latency = комфортно
- ✅ Rollback < 5 тиков назад (при 64Hz = 78ms)
- ✅ Можно позвать друга потестить

### Риски:
- ⚠️ Rapier BVH может быть недетерминистичен → fallback на Plan B (custom spatial hash)
- ⚠️ Fixed-point arithmetic может потребоваться раньше

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
