# VOIDRUN: Physics & Combat Architecture Design Document

## ⚠️ АРХИТЕКТУРА ИЗМЕНЕНА: См. ADR-003

**Дата решения:** 2025-01-10
**Новый подход:** Hybrid Architecture (ECS strategic + Godot tactical)

---

## Дата создания: 2025-10-07
## Последнее обновление: 2025-01-10 (HYBRID + Strategic Positioning)
## Версия: 3.1
## Статус: ~~ECS Authoritative~~ → **Hybrid (Godot-centric + StrategicPosition)**

---

## 0. Актуальная архитектура (2025-01-10)

### ✅ Принятое решение: Hybrid (Strategic ECS + Tactical Godot)

**См. полное обоснование:** [ADR-003: ECS vs Godot Physics Ownership](../decisions/ADR-003-ecs-vs-godot-physics-ownership.md)

**Краткое резюме:**

```
┌─────────────────────────────────────┐
│ Bevy ECS (Strategic Layer)          │
│ - Authoritative game state          │
│ - AI decisions, combat rules        │
│ - World simulation (economy, etc)   │
│ - Strategic position (zones)        │
└─────────────────────────────────────┘
           ↓ commands ↑ events
┌─────────────────────────────────────┐
│ Godot (Tactical Layer)              │
│ - Authoritative transform           │
│ - Physics (CharacterBody3D)         │
│ - Combat execution (animations)     │
│ - Pathfinding (NavigationAgent3D)   │
└─────────────────────────────────────┘
```

**Ключевые изменения:**
- ❌ Rapier больше НЕ используется для movement (опционален для queries)
- ✅ Godot Physics authoritative для Transform (tactical layer)
- ✅ ECS owns StrategicPosition (strategic layer) — ADR-005
- ✅ Синхронизация редкая (commands + events, 0.1-1 Hz)
- ✅ Chunk-based world для procgen — ADR-006

**Почему:**
- Single-player priority → детерминизм не критичен
- Client-Server netcode (не P2P rollback) → не требует bit-perfect physics
- Godot features (NavigationAgent3D, AnimationTree) → меньше кода
- Фокус на systems (economy, AI, quests) → точная физика не критична
- Procgen levels → NavMesh определяет spawn positions (не ECS)

**Transform Ownership:**
- **Godot Transform** (tactical) — authoritative для physics, rendering, pathfinding
- **ECS StrategicPosition** (strategic) — authoritative для AI/quests/economy

**См. также:**
- [ADR-005: Transform Ownership & Strategic Positioning](../decisions/ADR-005-transform-ownership-strategic-positioning.md)
- [ADR-006: Chunk-based Streaming World](../decisions/ADR-006-chunk-based-streaming-world.md)

---

## 1. Фундаментальное решение: Разделение симуляции и визуализации (ОРИГИНАЛЬНЫЙ ДИЗАЙН)

### ⚠️ Этот раздел описывает СТАРЫЙ подход (ECS Authoritative)

**Актуальный подход:** См. ADR-003

### Рекомендация (УСТАРЕЛА)

~~**Rust/Bevy ECS** — authoritative детерминистичная симуляция (геймплейная физика, hit detection, урон)~~
~~**Godot Physics** — презентационный слой (рагдоллы, обломки, интерполяция, client-side prediction)~~

**Новая рекомендация (2025-01-10):**
- **Bevy ECS** — authoritative game state (health, AI, economy)
- **Godot Physics** — authoritative transform + physics execution

### Обоснование из индустрии (2024-2025)

**Успешные решения:**
- **Photon Quantum 3** (Unity Verified Solution 2024): полностью детерминистичный ECS движок с rollback netcode, fixed-point math
- **Box2D 2024**: достиг cross-platform детерминизма **без** fixed-point (избегая fast-math, FMA, custom atan2f)
- **SG Physics 2D** (Godot, Aug 2025): fixed-point 2D физика специально для rollback netcode
- **Mirror Networking** (2024): active development на lag compensation + client-side prediction для shooters

**Ключевые инсайты:**
1. Rollback netcode требует bit-perfect детерминизма → нельзя полагаться на Godot/Unity Physics (недетерминированны)
2. Modern CPU (64-bit, SIMD) обеспечивают консистентную арифметику → fixed-point не обязателен (если избегать fast-math/FMA)
3. Client-side prediction критична для "feel" в высокодинамичных играх → Godot должен предсказывать локально

### Trade-offs

✅ **За разделение:**
- Rollback netcode физически возможен (Photon Quantum подход)
- Headless симуляция для CI и dedicated servers
- Кросс-платформенный детерминизм (одинаковый Rust binary)
- Защита от читерства (сервер не доверяет клиенту)
- Портируемость фронтенда (можно заменить Godot)

⚠️ **Против:**
- Дублирование: синхронизация Rust Transform ↔ Godot Node3D
- Своя collision detection (но есть `bevy_rapier` в kinematic режиме)
- Latency требует lag compensation (дополнительная сложность)

---

## 2. Детерминизм: Fixed-Point vs Float Arithmetic

### Современное состояние (2024-2025)

**Традиционный взгляд (до 2020-х):**
> Cross-platform детерминизм = обязательно fixed-point math

**Современная реальность:**
> 64-bit CPU с консистентным IEEE-754 rounding → float детерминизм возможен **при правильных условиях**

### Box2D 2024 подход (без fixed-point)

**Что делать:**
- ❌ Отключить `fast-math` compiler флаг
- ❌ Избегать FMA (Fused Multiply-Add) инструкции
- ✅ Реализовать custom `atan2f` (стандартная либа недетерминистична)
- ✅ Использовать одинаковый компилятор/версию для всех платформ

**Результат:** кросс-платформенный детерминизм без жертв производительности fixed-point

### Photon Quantum 3 подход (fixed-point)

**Формат:** Кастомный `FP` struct заменяет все float/double
**Плюсы:** 100% гарантия детерминизма, не зависит от компилятора
**Минусы:** нельзя использовать сторонние библиотеки (включая physics engines, pathfinding)

### Рекомендация для VOIDRUN (ОБНОВЛЕНО 2025-01-10)

**⚠️ Fixed-point НЕ НУЖЕН для Hybrid Architecture**

**Причины:**
- Single-player priority → rollback не нужен
- Client-Server netcode → snapshot interpolation, не determinism
- Saves/loads → достаточно f32/f64

**Новый подход (упрощённый):**

**ECS (game state):**
- Health, Stamina, Inventory → **f32** (достаточная точность)
- Economy prices, AI weights → **f64** (для больших чисел)
- RNG → seeded `ChaCha8Rng` (для procedural generation)
- Strategic position → `ZoneId` (enum, не coordinates)

**Godot (physics):**
- Transform → **f32** (Godot стандарт)
- Velocity, acceleration → **f32**
- Collision detection → Godot Physics (недетерминистично, но OK)

**Когда вернуться к fixed-point:**
- Если решим делать P2P rollback netcode (маловероятно)
- Если найдём критичные float bugs (пока нет)

**Компромисс (СТАРЫЙ, опционально):**
~~- Использовать `bevy_rapier` для collision queries (kinematic режим), но конвертировать результаты через fixed-point при применении урона~~
~~- Если обнаружим десинхроны — мигрировать критичные части в full fixed-point~~

**Актуальный:** Godot Physics, f32 везде, простота > детерминизм

---

## 3. Collision Detection (ОБНОВЛЕНО: Godot Physics)

### ⚠️ Rapier опционален в Hybrid Architecture

**Актуальное решение (2025-01-10):**
- Godot Physics для всех collisions (CharacterBody3D, Area3D)
- Rapier НЕ используется для movement
- Опционально: Rapier queries для ECS logic (если понадобится)

**См. детали:** [ADR-003](../decisions/ADR-003-ecs-vs-godot-physics-ownership.md)

---

## 3.1. Collision Detection: Rapier в детерминистичном режиме (СТАРЫЙ ДИЗАЙН)

### bevy_rapier Capabilities (2024)

**CCD (Continuous Collision Detection):**
- Swept sphere метод для быстрых объектов (снаряды 1000+ m/s)
- Компонент: `Ccd::enabled()` на projectile entity
- Nonlinear CCD: учитывает вращение + перемещение
- `max_ccd_substeps` для точности (default 1, увеличить для критичных случаев)

**Kinematic режим для детерминизма:**
- `RigidBody::KinematicPositionBased` — мы двигаем, Rapier только queries
- `RapierConfiguration { physics_pipeline_active: false, query_pipeline_active: true }`
- Отключить гравитацию: `gravity: Vec3::ZERO`
- Deterministic flag: `RapierConfiguration::deterministic = true`

**Query API:**
- `QueryPipeline::cast_ray()` — hitscan оружие, raycast к hitbox'ам
- `QueryPipeline::cast_shape()` — swept collision для melee (capsule vs OBB)
- `QueryPipeline::intersections_with_shape()` — AOE эффекты

**⚠️ КРИТИЧЕСКИЙ РИСК: BVH Non-Determinism (аудит 2025)**

**Проблема:**
- Rapier BVH tree может давать **разный порядок** результатов при параллельных queries
- Даже с `deterministic = true` флагом, BVH rebuild может быть недетерминистичным
- Параллельный broad-phase (SIMD) может менять порядок на разных CPU

**Обнаружение:**
- CI тест: 1000 replay runs на разных CPU (Intel/AMD/Apple Silicon)
- Критерий провала: хотя бы один desync в финальных позициях
- Метрика: max position diff должно быть **0 fixed-point units**, не <0.001

**Plan B (если Rapier провалит тесты):**
- Использовать `rapier3d = { features = ["single-threaded"] }` — отключить параллелизм
- Или: custom spatial hash (HashMap с fixed-point ключами) + manual broad-phase
- Или: parry3d напрямую (без Rapier pipeline) для queries

### Plan B: Custom Spatial Hash (если Rapier провалит детерминизм)

**Когда переходить на Plan B:**
- Если CI детерминизм-тесты обнаружат desync (хотя бы 1 из 1000 runs)
- Если Rapier BVH rebuild даёт разные результаты на разных CPU
- Критерий: max position diff >0 fixed-point units

**Архитектура Custom Spatial Hash:**
1. **Broad-phase:** HashMap с fixed-point grid ключами
   - Key: `(x / GRID_SIZE, y / GRID_SIZE, z / GRID_SIZE)` в fixed-point
   - Value: `Vec<Entity>` в детерминистичном порядке (sorted by Entity StableId)
2. **Narrow-phase:** `parry3d` queries для геометрии (ray-sphere, ray-capsule, ray-OBB)
   - parry3d детерминистичен если не использовать его pipeline
3. **Update:** при изменении Position → reinsert в grid (O(1))

**Trade-offs:**
- ✅ 100% детерминизм (контролируем порядок везде)
- ✅ Прозрачность (можем debug логировать все queries)
- ⚠️ Медленнее на старте (нет оптимизированного BVH, но достаточно для MVP)
- ⚠️ Больше кода (~500 строк vs Rapier out-of-box)

**Миграция cost:** 1-2 недели (если начать после Фазы 2)

---

## 4. Hierarchical Hitbox System

### Архитектура компонентов (Bevy ECS)

**Entity структура:**

```
ShipEntity / CharacterEntity
├─ GlobalTransform (authoritative, fixed-point Position)
├─ Velocity (fixed-point Vec3)
├─ Health (total entity health)
└─ Children (Bevy hierarchy):
    ├─ HitboxPartEntity (ShipEngine)
    │   ├─ HitboxPart { shape: Sphere, health, armor, damage_multiplier: 2.0 }
    │   ├─ LocalTransform (offset от родителя)
    │   └─ RapierCollider::ball(radius) [для queries]
    ├─ HitboxPartEntity (ShipReactor)
    │   ├─ HitboxPart { shape: OBB, ... }
    │   └─ ...
    └─ HitboxPartEntity (ShipWeaponMount)
        └─ ...
```

**Для персонажей:**
- `Head` (multiplier 3.0, малый radius)
- `Torso` (multiplier 1.0, capsule)
- `LeftArm` / `RightArm` (multiplier 0.5, capsules)
- `LeftLeg` / `RightLeg` (multiplier 0.7, capsules)

### Компоненты (текстовое описание)

**HitboxPart:**
- `parent_entity`: Entity (корабль/персонаж)
- `part_type`: Enum (Engine, Reactor, Head, Torso, Limb)
- `shape`: Enum { Sphere(r), Capsule(r, h), OBB(half_extents) }
- `health`: u32
- `max_health`: u32
- `armor`: u32
- `damage_multiplier`: fixed-point (критичность зоны)
- `destruction_effects`: Vec<EffectId> (что происходит при уничтожении)
- `disabled`: bool (часть уничтожена, но entity жива для визуальных эффектов)

**WeaponSystem:**
- `projectile_speed`: fixed-point
- `fire_rate_hz`: fixed-point
- `spread_angle`: fixed-point (cone of fire)
- `damage`: u32
- `armor_penetration`: u32
- `weapon_type`: Enum (Hitscan, Projectile, Melee)

### Best Practices из индустрии

**Counter-Strike: GO подход:**
- Capsule hitbox'ы вместо boxes (меньше false negatives)
- Hitbox'ы внутри визуальной модели (не торчат наружу)
- Bulky rectangles для головы (попадание в край шлема = headshot)

**Modern MOBA shooters:**
- Большие capsules (выстрел между коленей = попадание)
- Trade-off: feel vs fairness (generous hitbox'ы для лучшего feedback)

**Рекомендация для VOIDRUN:**
- **Космос:** Sphere hitbox'ы для ship parts (двигатели, реакторы) — просто, быстро, достаточно
- **FPS на земле:** Capsule для торса/конечностей, Sphere для головы (баланс precision vs performance)
- **Melee:** OBB (oriented bounding box) для оружия swing arc

---

## 5. Прицельная стрельба: Swept Collision для быстрых снарядов

### Проблема tunneling

Снаряд 1000 m/s, timestep 1/64 сек → за тик пролетает 15.6 метров
Мелкий hitbox (голова = 0.2m radius) легко "прошивается" между тиками

### Решение: CCD с Rapier

**Алгоритм (FixedUpdate 64Hz):**

1. **ProjectileMovementSystem:**
   - Сохранить `prev_position` в компоненте
   - Обновить `position += velocity * FIXED_TIMESTEP`
   - Конвертировать в fixed-point

2. **HitDetectionSystem (после ProjectileMovement):**
   - Для каждого projectile: `QueryPipeline::cast_ray(prev_pos, current_pos)`
   - Rapier вернёт все пересечения с `HitboxPart` colliders
   - Выбрать closest hit по `time_of_impact` (TOI ∈ [0, 1])
   - Событие: `ProjectileHit { projectile, target, part_entity, hit_point, normal }`

3. **DamageApplicationSystem (Exclusive, после HitDetection):**
   - Читать события `ProjectileHit`
   - Расчёт: `effective_damage = (weapon.damage - hitbox.armor) * hitbox.damage_multiplier`
   - Атомарно вычесть `hitbox.health -= effective_damage`
   - Если `hitbox.health <= 0` → событие `HitboxPartDestroyed { entity, part_type }`

### Детерминизм гарантии

- Все позиции в fixed-point (u64 представление координат)
- Rapier в kinematic режиме (мы двигаем projectile, не dynamic physics)
- Explicit ordering: `ProjectileMovement.before(HitDetection).before(DamageApplication)`
- TOI comparison: использовать OrderedFloat или сравнение на fixed-point

### Оптимизация из индустрии

**Spatial partitioning:**
- При 100+ снарядах: использовать broadphase (BVH tree встроен в Rapier)
- Дополнительно: LOD для дальних hitbox'ов (объединить части в один AABB на дистанции >100m)

**Secondary hitbox'ы (оптимизация из поиска):**
- В напряжённых боях (50+ entities стреляют): временно использовать упрощённые hitbox'ы (один capsule вместо дерева)
- Переключение по heuristic: "если в радиусе 20m больше 10 активных projectiles → упрощённый режим"

---

## 6. Melee Combat: Парирование и timing windows

### Best Practices из Fighting Games (2024)

**Timing windows (из поиска Samurai Shodown, Sekiro):**
- **Парирование:** ~0.5 сек окно, обычно 0.3-0.7 сек в зависимости от сложности
- **Visual cues:** flash за 0.1-0.2 сек до активного окна (подготовка игрока)
- **Audio cues:** звук замаха → rhythm game feel (игрок считает в уме)
- **Penalty за промах:** cooldown 1-2 сек уязвимости (чтобы не было spam parry)

**Strategic depth:**
- Движение манипулирует timing: walk backward → атака входит в parry window позже
- Baiting: притвориться что атакуешь → противник парирует рано → punish
- Mind games: delay атаки чтобы обмануть парирование

### Архитектура в Bevy ECS

**Компоненты:**

**MeleeWeapon:**
- `swing_duration_ticks`: u32 (длина анимации, например 20 тиков при 64Hz = 0.31 сек)
- `damage_window`: [start_tick, end_tick] (когда hitbox активен, например [10, 15])
- `parry_window`: [start_tick, end_tick] (когда можно парировать, например [8, 12])
- `swing_arc_degrees`: fixed-point (конус атаки, 90 degrees для меча)
- `reach`: fixed-point (дальность)
- `damage`: u32

**CombatState:**
- `state`: Enum { Idle, Swinging { started_tick, weapon }, Parrying { started_tick }, Stunned { until_tick } }
- `facing_direction`: fixed-point Vec3
- `last_attacked_tick`: u64 (для cooldown)

**Системы (FixedUpdate):**

**MeleeInputSystem:**
- Читает input (AttackPressed, ParryPressed)
- Проверяет: не в Stunned? cooldown прошёл?
- Переводит в Swinging / Parrying, записывает `started_tick = current_tick`

**MeleeAttackSystem (после Input):**
- Для всех Swinging entities:
  - Проверить: `current_tick - started_tick` в `damage_window`?
  - Cone check: `QueryPipeline::intersections_with_shape()` (capsule в направлении facing)
  - Для каждого потенциального target в cone:
    - Если target в `Parrying` state + facing к атакующему (dot product > 0.7) → **Parry Success**
      - Событие: `AttackParried { attacker, defender }`
      - Атакующий → `Stunned { until_tick: current + 60 }` (1 сек при 64Hz)
    - Иначе → raycast к hitbox дереву target, применить урон к ближайшей части

**ParryTimingSystem (после Attack):**
- Для всех Parrying entities:
  - Проверить: `current_tick - started_tick > parry_window.end`?
  - Если окно истекло без атаки → cooldown (idle с `last_parry_failed_tick`)

### Детерминизм

- Cone check: `dot(facing, to_target) >= cos(swing_arc / 2)` — fixed-point trig через lookup table
- Swept capsule (оружие от прошлого тика до текущего): Rapier `cast_shape` с Capsule
- Timing: tick counters (u64) абсолютно детерминистичны

---

## 7.Lag Compensation & Client-Side Prediction

### Индустрия стандарт (2024)

**Mirror Networking подход:**
- **Client-side prediction:** клиент мгновенно применяет input (0 lag feel)
- **Server reconciliation:** сервер подтверждает или корректирует (rollback если расхождение)
- **Lag compensation:** сервер "rewind" мира на ping игрока для hit detection

**Условие работы:**
- Latency игроков не должен сильно различаться (разница >100ms = disadvantage для low-ping игроков)
- Используется в fast-paced PvP shooters

**Photon Quantum rollback:**
- Все клиенты предсказывают inputs других игроков
- Сервер отправляет confirmed inputs → клиенты rollback + re-simulate
- Требует bit-perfect детерминизм

### Реализация для VOIDRUN

**Уровень 1: Client-side prediction (для плавности):**

**Godot слой:**
1. Локальный игрок нажимает кнопку выстрела
2. Godot немедленно:
   - Показывает muzzle flash
   - Трассирует луч от камеры
   - Если пересечение с hitbox → показать blood/sparks
   - Отправляет input в Rust: `FireCommand { client_tick, aim_direction }`

3. Rust сервер (через 50ms):
   - Получает `FireCommand`
   - Выполняет authoritative hit detection (с lag compensation)
   - Отправляет обратно: `HitConfirmed { hit: bool, target, damage }` или `HitDenied`

4. Godot reconciliation:
   - Если `HitConfirmed` → оставить эффект
   - Если `HitDenied` → убрать кровь/искры, показать "MISS" indicator

**Уровень 2: Lag compensation на сервере (для справедливости):**

**Rust реализация:**
1. Сервер хранит history: `PositionHistory<Entity> = RingBuffer<(tick, GlobalTransform)>`
2. При получении `FireCommand { client_tick, ... }`:
   - Вычислить latency: `server_tick - client_tick`
   - "Rewind" hitbox'ы target'а на `latency` тиков назад
   - Выполнить hit detection против прошлых позиций
   - Результат: клиент видит "попадание" там где целился (компенсация лага)

**Trade-offs:**
- ✅ Low-ping игрок не чувствует лаг
- ⚠️ High-ping игрок может "попадать за углом" (target уже спрятался на своём экране)
- ⚠️ История позиций: нужно хранить ~2 сек (128 тиков при 64Hz) × количество hitbox'ов

### Rollback netcode (для co-op PvE)

**Архитектура:**
- Использовать GGRS (Rust rollback библиотека, совместима с Bevy)
- Все клиенты симулируют одинаково (детерминизм обязателен)
- При получении inputs от других игроков → rollback N тиков + re-simulate
- Визуальная сторона (Godot) показывает только последний confirmed state

**Когда использовать:**
- 2-4 игрока co-op (небольшое количество)
- PvE (меньше соревновательности, можно терпеть редкие rollback'и)
- НЕ для PvP (визуальные rollback'и раздражают)

---

## 8. Godot визуализация и интерполяция

### Роль Godot в архитектуре

**Godot НЕ отвечает за:**
- ❌ Геймплейные позиции (authoritative в Rust)
- ❌ Hit detection (делается в Rust)
- ❌ Урон и health (Rust)

**Godot ОТВЕЧАЕТ за:**
- ✅ Визуальные позиции (интерполяция между Rust updates)
- ✅ Анимации (синхронизированы с Rust CombatState)
- ✅ Client-side prediction (локальный игрок двигается мгновенно)
- ✅ Visual effects (кровь, искры, обломки — локально, не влияет на Rust)
- ✅ Ragdolls и обломки (Godot Physics, не детерминистично, только презентация)

### Интерполяция между Rust updates

**Проблема:** Rust шлёт позиции 64 раза/сек → между updates 15.6ms → может быть видно "телепортацию"

**Решение: Client-side интерполяция (GDScript):**

```
(Текстовое описание алгоритма)

1. Rust отправляет: PositionUpdate { entity_id, position, rotation, tick }
2. Godot хранит: previous_position, current_position, next_position (triple buffer)
3. Каждый Godot frame (Update, ~60fps):
   - t = (Time.get_ticks_msec() - last_update_time) / FIXED_TIMESTEP_MS
   - interpolated_pos = lerp(current_position, next_position, t)
   - Node3D.transform.origin = interpolated_pos
4. При получении нового update: previous = current, current = next, next = new
```

**Trade-off:**
- ✅ Плавное движение (не видно дискретных скачков)
- ⚠️ Визуальная задержка ~1 tick (15ms при 64Hz) — приемлемо

### Ragdolls и обломки (Godot Physics)

**Workflow при уничтожении части:**

1. Rust шлёт событие: `HitboxPartDestroyed { entity, part_type: ShipEngine, position, velocity }`
2. Godot получает:
   - Играет анимацию отрыва (двигатель отваливается)
   - Спавнит RigidBody3D с визуальной моделью обломка
   - Применяет начальную velocity из события
   - Godot Physics симулирует падение/отскоки локально (не отправляется в Rust)
3. Через 5-10 сек обломок despawn'ится (или при уходе из view distance)

**Критично:** обломки НЕ имеют collision с геймплейными entities (только с terrain для визуала)

### Анимации синхронизация

**Rust → Godot события:**
- `CombatStateChanged { entity, state: Swinging, started_tick }`
- `AnimationTrigger { entity, animation: "sword_slash", start_frame }`

**Godot AnimationTree:**
- Получает событие → transition в нужный state
- `AnimationPlayer.seek()` к правильному frame (если событие пришло с задержкой)
- Blend между состояниями при rollback (если предсказание было неверным)

---

## 9. Риски и метрики

### Критические риски

**Десинхронизация Rust ↔ Godot:**
- **Симптом:** Godot показывает попадание, Rust говорит промах (или наоборот)
- **Обнаружение:**
  - Лог: каждое расхождение (predicted hit ≠ confirmed hit)
  - Метрика: `prediction_accuracy` = confirmed / predicted, target >95%
- **Решение:** Godot визуально показывает "под вопросом" (слабая искра) → яркий эффект только после confirm

**Детерминизм нарушен:**
- **Симптом:** Replay десинхронизировался, два одинаковых боя дали разный исход
- **Обнаружение:**
  - CI property test: одинаковый seed → bit-identical snapshots
  - Запись desync logs: сравнивать world state между клиентами каждые 10 тиков
- **Решение:**
  - Проверить fast-math флаги компилятора
  - Проверить порядок систем (explicit `.chain()`)
  - Профилировать RNG usage (не должно быть неконтролируемых вызовов)

**Performance bottleneck в HitDetection:**
- **Симптом:** p95 latency HitDetectionSystem >5ms (budget 8ms при 120Hz)
- **Обнаружение:** Tracy профайлер, bevy diagnostic
- **Решение:**
  - Включить LOD: упростить hitbox'ы для дальних entities (>50m)
  - Spatial culling: не проверять entities вне view frustum игроков
  - Broadphase оптимизация: убедиться что Rapier BVH tree корректно работает

**Rollback визуальные глитчи:**
- **Симптом:** При rollback анимация "дёргается", игрок видит телепортацию
- **Обнаружение:** Субъективно в playtesting, метрика: частота rollback'ов >5/сек
- **Решение:**
  - Увеличить input buffer (предсказывать дальше в будущее)
  - Smooth blend в Godot AnimationTree между rollback'нутой и актуальной позой
  - Ограничить rollback depth (не откатывать дальше 10 тиков)

### Целевые метрики

**Performance (Rust FixedUpdate 64Hz):**
- ProjectileMovementSystem: p95 <1ms, p99 <2ms
- HitDetectionSystem: p95 <3ms, p99 <5ms
- DamageApplicationSystem: p95 <1ms (exclusive, блокирует)
- **Total FixedUpdate:** p95 <8ms, p99 <12ms (для 120Hz = 8.3ms period)

**Netcode quality:**
- Client prediction accuracy: >95% (confirmed hits / predicted hits)
- Rollback frequency: <2 rollbacks/sec в co-op (при ping <100ms)
- Lag compensation fairness: win rate не коррелирует с ping (±5% допустимо)

**Детерминизм:**
- Replay convergence: 100% (одинаковый seed → bit-identical outcome)
- Save/load stability: `diff(save(load(state)), state) == ∅`

**Gameplay balance:**
- Parry success rate: 60-70% при правильном timing (не слишком легко/сложно)
- TTK (time to kill): среднее 3-5 сек в FPS (без weak spot), 1-2 сек при headshot
- Weak spot эффективность: 2-3x damage multiplier (баланс skill vs frustration)

---

## 10. План внедрения (поэтапный)

### Фаза 0: Foundation (1 неделя)

**Задачи:**
1. Добавить в Cargo.toml:
   - `bevy_rapier3d` с features `["simd-stable", "parallel", "enhanced-determinism"]`
   - `fixed` crate (>= 1.28) для fixed-point арифметики
   - `rand_chacha` для детерминистичного RNG
   - `bevy = "0.16"` (не 0.14!)
2. Настроить compiler для детерминизма:
   - Создать `.cargo/config.toml`:
     ```toml
     [build]
     rustflags = ["-C", "target-feature=-fma"]
     ```
   - Это отключает Fused Multiply-Add (недетерминистичны на разных CPU)
3. Настроить Rapier:
   - Kinematic режим, отключить динамику
   - `RapierConfiguration { deterministic: true, gravity: Vec3::ZERO }`
4. Базовые компоненты:
   - `Position` (fixed-point Vec3), `Velocity`, `Health`
5. Headless тест: spawn 100 entities, двигать 1000 тиков, проверить детерминизм
6. **КРИТИЧЕСКИЙ тест:** запустить на разных CPU (Intel/AMD) → результаты должны быть bit-identical

**Критерий готовности:**
- CI прогоняет headless симуляцию 1000 тиков, **10 запусков** дают bit-identical snapshots
- Тесты на Intel и AMD дают одинаковый результат (для проверки FMA отключения)

---

### Фаза 1: Hitbox Tree System (1-2 недели)

**Задачи:**
1. Компонент `HitboxPart` с полями из раздела 4
2. Система `SpawnHitboxTreeSystem`:
   - При создании Ship/Character → spawn children entities с HitboxPart
   - Каждый HitboxPart имеет `RapierCollider::ball()` / `capsule()` / `cuboid()`
   - LocalTransform задаёт offset относительно parent
3. Godot интеграция:
   - GDExtension функция: `get_hitbox_tree(entity_id) -> Array[HitboxPartData]`
   - Godot создаёт визуальные маркеры (MeshInstance3D, highlight при прицеливании)
4. Тест: spawn корабль с 5 hitbox'ами, запросить из Godot, проверить структуру

**Критерий готовности:** Godot получает иерархию hitbox'ов, рисует debug spheres/capsules, они двигаются вместе с entity

---

### Фаза 2: Прицельная стрельба + CCD (2 недели)

**Задачи:**
1. Компоненты:
   - `Projectile { prev_pos, velocity, damage, lifetime_ticks }`
   - `WeaponSystem` из раздела 4
2. Системы:
   - `FireWeaponSystem`: при input создаёт projectile entity с `Ccd::enabled()`
   - `ProjectileMovementSystem`: обновляет позицию (fixed-point), конвертирует в `RapierTransform`
   - `HitDetectionSystem`: `QueryPipeline::cast_ray(prev, current)` → выбор closest hit
   - `DamageApplicationSystem` (Exclusive): применяет урон, генерирует `HitboxPartDestroyed` события
3. Godot client-side prediction:
   - При выстреле: немедленно показать tracer + искры
   - При получении `HitConfirmed` → усилить эффект
   - При `HitDenied` → убрать искры, показать "MISS"
4. Тест: бот стреляет в неподвижную цель 1000 раз, проверить hit rate, детерминизм

**Критерий готовности:** Headless тест с 10 ботами стреляющими друг в друга проходит детерминистично, Godot плавно показывает попадания

---

### Фаза 3: Melee Combat + Парирование (1-2 недели)

**Задачи:**
1. Компоненты: `MeleeWeapon`, `CombatState` из раздела 6
2. Системы:
   - `MeleeInputSystem`: input → Swinging / Parrying state transitions
   - `MeleeAttackSystem`: cone check + swept capsule, parry detection
   - `ParryTimingSystem`: проверка timing windows, stunned penalties
3. События:
   - `AttackParried { attacker, defender }`
   - `MeleeHit { attacker, target, part, damage }`
4. Godot анимации:
   - События Rust → AnimationTree state transitions
   - Blend при rollback (если prediction неверен)
5. Тест: два бота дерутся 100 раундов, метрики:
   - Parry success rate ~65% (бот с идеальным timing)
   - TTK среднее ~4 сек
   - Детерминизм: одинаковый seed → одинаковый победитель

**Критерий готовности:** Headless бои воспроизводятся детерминистично, Godot плавно показывает parry анимации с правильным timing

---

### Фаза 4: Lag Compensation (2 недели)

**Задачи:**
1. `PositionHistory` ресурс: `HashMap<Entity, RingBuffer<(tick, Transform)>>`
2. `RecordPositionSystem` (Last в FixedUpdate): сохраняет позиции всех hitbox'ов
3. `LagCompensatedHitDetection`:
   - При получении `FireCommand { client_tick }`:
     - Вычислить latency = `server_tick - client_tick`
     - Rewind hitbox'ы на latency тиков
     - Выполнить raycast против прошлых позиций
   - Отправить обратно: `HitConfirmed` / `HitDenied`
4. Godot reconciliation (из фазы 2)
5. Тест с симуляцией лага:
   - Добавить искусственную задержку 50-150ms
   - Проверить что prediction accuracy >95%
   - Проверить fairness: win rate не зависит от ping (±5%)

**Критерий готовности:** Playtesting с искусственным лагом ощущается responsive, метрики prediction accuracy в зелёной зоне

---

### Фаза 5: Rollback Netcode для Co-op (2-3 недели, опционально)

**Задачи:**
1. Интеграция GGRS crate (Rust rollback библиотека)
2. Snapshot/restore через bevy_save (уже в плане из bevy-ecs-design.md)
3. Input serialization: все player inputs → deterministic stream
4. Rollback loop:
   - При получении remote input → compare с prediction
   - Если расхождение → rollback N тиков + re-simulate
   - Godot получает только confirmed state
5. Тест: 4 игрока co-op с симуляцией packet loss (5%)
   - Проверить convergence (все клиенты видят одинаковый результат через 1-2 сек)
   - Метрика: rollback frequency <2/sec

**Критерий готовности:** Co-op сессия с 4 игроками работает стабильно при ping <100ms и packet loss <5%

---

### Фаза 6: Optimizations + Балансировка (ongoing)

**Задачи:**
1. Профилирование Tracy: найти bottleneck'и в HitDetection
2. LOD hitbox'ов: дальние entities (>50m) получают упрощённые hitbox'ы
3. Spatial culling: не проверять entities вне player view frustum
4. Balancing через headless CI:
   - 1000 rounds бои ботов с разными тактиками
   - Метрики: TTK, parry success rate, weak spot effectiveness
   - Корректировать damage_multiplier, armor, parry_window_ticks
5. Godot visual polish:
   - Улучшить интерполяцию (cubic spline вместо linear lerp)
   - Добавить motion blur для быстрых движений
   - Ragdoll tuning (физические параметры для красивого падения)

**Критерий готовности:** FixedUpdate p95 <8ms, prediction accuracy >95%, субъективно gameplay ощущается tight and responsive

---

## 11. Инварианты для CI проверки

**После каждого FixedUpdate тика:**
- Сумма health всех HitboxPart entity <= total entity health (ассерт)
- Destroyed HitboxPart имеют health == 0 и disabled == true (ассерт)
- Projectile не существует дольше max_lifetime тиков (cleanup проверка)
- CombatState transitions валидны: нет прыжков Idle → Stunned без Swinging (state machine validation)
- Все Position компоненты в valid range (проверка на NaN, overflow fixed-point)

**Детерминизм property tests:**
- `world_before → apply_inputs → world_after`, повторить 10 раз → bit-identical результаты
- `save(state) → load() → save() → compare` → файлы побайтово одинаковые
- Replay: записать 1000 тиков inputs → переиграть → одинаковый финальный snapshot
- **Cross-CPU test (КРИТИЧНЫЙ):** одинаковый seed на Intel/AMD/Apple Silicon → bit-identical результаты
  - Если провалит → Rapier недетерминистичен → активировать Plan B (custom spatial hash)
  - Критерий: max position diff = **0** (не <0.001!)

**Balance invariants:**
- Parry success rate при идеальном timing: 60-80% (не слишком easy/hard)
- TTK median: 3-5 сек (без weak spots), 1-2 сек (headshot/critical)
- Weak spot damage multiplier: среднее 2-3x (не должно превышать 5x, чтобы не было frustration)
- Armor effectiveness: должен уменьшать damage на 20-60% (не должен делать invincible)

**Netcode качество (в playtesting logs):**
- Client prediction accuracy: >95%
- Rollback frequency: <2/sec при ping <100ms
- Position desync: <0.5m между клиентами (должны converge за 1 сек)

---

## 12. Ссылки и источники

### Детерминизм и Rollback (2024-2025)
- **Photon Quantum 3** (Unity Verified Solution): https://www.photonengine.com/quantum
- **Box2D Determinism** (Aug 2024): https://box2d.org/posts/2024/08/determinism/
- **SG Physics 2D** (Godot, Aug 2025): https://www.snopekgames.com/project/sg-physics-2d/
- **Delta Rollback** (Godot plugin, July 2024): https://medium.com/@david.dehaene/delta-rollback-new-optimizations-for-rollback-netcode-7d283d56e54b

### Lag Compensation & Prediction
- **Mirror Networking** (2024): https://mirror-networking.gitbook.io/docs/manual/general/lag-compensation
- **Gabriel Gambetta** (классика): https://www.gabrielgambetta.com/client-side-prediction-server-reconciliation.html
- **Valve Source Engine**: https://developer.valvesoftware.com/wiki/Latency_Compensating_Methods_in_Client/Server_In-game_Protocol_Design_and_Optimization

### Collision Detection
- **Rapier CCD docs**: https://rapier.rs/docs/user_guides/bevy_plugin/rigid_body_ccd/
- **Bullet CCD**: https://docs.panda3d.org/1.10/python/programming/physics/bullet/ccd
- **Swept Collision theory**: https://digitalrune.github.io/DigitalRune-Documentation/html/138fc8fe-c536-40e0-af6b-0fb7e8eb9623.htm

### Hitbox Design
- **Counter-Strike hitbox evolution**: https://counterstrike.fandom.com/wiki/Hitbox
- **Hitboxes & Hurtboxes in Unity**: https://www.gamedeveloper.com/design/hitboxes-and-hurtboxes-in-unity

### Fixed-Point Arithmetic
- **Gaffer On Games** (floating-point determinism): https://gafferongames.com/post/floating_point_determinism/
- **Cross-platform RTS sync**: https://www.gamedeveloper.com/programming/cross-platform-rts-synchronization-and-floating-point-indeterminism
- **Random ASCII** (Bruce Dawson): https://randomascii.wordpress.com/2013/07/16/floating-point-determinism/

---

## 13. Открытые вопросы для будущих итераций

### Fixed-Point библиотека
**Вопрос:** Использовать `fixed` crate, `fxp`, или custom implementation?
**Рекомендация:** Начать с `fixed` (более mature), мигрировать на `fxp` если нужна лучшая производительность SIMD

### Rollback vs Lag Compensation для PvP
**Вопрос:** В будущем PvP (arena 4v4) — rollback или lag compensation?
**Рекомендация:** Lag compensation (Mirror подход) — rollback визуально дёргается в PvP, игроки предпочитают smooth с небольшим лагом

### Rapier детерминизм проверка
**Вопрос:** Действительно ли Rapier в kinematic режиме даст bit-perfect детерминизм?
**Статус (аудит 2025):** ⚠️ **НЕ ГАРАНТИРОВАН** — BVH rebuild может быть недетерминистичным
**План:**
1. Stress-test в фазе 0 (НЕ фазе 2!) — максимально рано обнаружить проблему
2. Тест: 1000 replay runs на Intel, AMD, Apple Silicon
3. Если хотя бы 1 desync → активировать Plan B (custom spatial hash)
4. **Fallback:** `features = ["single-threaded"]` если custom hash не нужен (trade-off: медленнее)

### LOD агрессивность
**Вопрос:** На какой дистанции упрощать hitbox'ы? 50m? 100m?
**План:** A/B тест с метриками: balance между performance (FPS) и gameplay (дальние headshot'ы должны работать)

---

**Финальная рекомендация:**
Начать с **Фаза 0-3** (Foundation → Hitbox Tree → Ranged Combat → Melee), это даст вертикальный срез всей системы. Lag compensation (Фаза 4) критична для multiplayer feel. Rollback (Фаза 5) можно отложить до первого co-op playtest'а.

Ключевой успех: **детерминизм с первого дня** — каждая фаза должна проходить property tests в CI, иначе rollback netcode физически невозможен.
