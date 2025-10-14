# Melee Combat System: Implementation Plan

**Дата начала:** 2025-01-13
**Статус:** 🚧 In Progress
**Фаза:** 2.0 - Weapon Architecture Refactoring
**Roadmap:** [Фаза 1.5 - Combat Mechanics](../roadmap.md#фаза-15-combat-mechanics-текущее)

---

## Обзор

Реализация полноценной системы ближнего боя с:
- Фазовыми атаками (windup → attack → recovery)
- Защитными механиками (block, parry, dodge)
- Умным AI (реакция на замах противника, выбор defensive options)
- Унифицированной архитектурой оружия (melee + ranged)

**Текущий статус:**
- ✅ Ranged combat работает (AI стреляет, projectiles летят)
- 🔴 Melee combat сломан (нет системы генерации атак)
- ⏸️ Defensive mechanics не начаты

**Milestone цель:** 2 NPC с мечами дерутся друг с другом, используют parry/dodge/block, AI разумно принимает решения.

---

## Архитектурные решения (Фаза 1)

### Решение 1: Unified WeaponStats (Вариант A) ✅

**Проблема:** Текущие компоненты раздроблены:
- `Attacker` (melee stats) в `combat/attacker.rs`
- `Weapon` (ranged stats) в `combat/weapon.rs`
- `Attachment` (visual prefab) в `components/attachment.rs`

**Решение:** Объединить `Attacker` + `Weapon` → `WeaponStats`.

**Обоснование:**
- ✅ Единый источник истины для weapon data
- ✅ Легко swapить оружие (одна замена компонента)
- ✅ Hybrid weapons (штык-нож) работают из коробки
- ✅ Меньше boilerplate кода

**Структура:**
```rust
#[derive(Component, Clone, Debug, Reflect)]
pub struct WeaponStats {
    pub weapon_type: WeaponType,  // Melee / Ranged / Hybrid
    pub base_damage: u32,
    pub attack_cooldown: f32,

    // Melee-specific
    pub attack_radius: f32,
    pub windup_duration: f32,
    pub attack_duration: f32,
    pub recovery_duration: f32,
    pub parry_window: f32,

    // Ranged-specific
    pub range: f32,
    pub projectile_speed: f32,
    pub hearing_range: f32,
}

pub enum WeaponType {
    Melee { can_block: bool, can_parry: bool },
    Ranged,
    Hybrid,
}
```

---

### Решение 2: WeaponStats → Attachment (Required Components) ✅

**Проблема:** Как связать weapon stats (ECS) с visual prefab (Godot)?

**Решение:** `WeaponStats` требует `Attachment`, но НЕ наоборот.

**Логика:**
- **Если есть `WeaponStats`** → обязательно есть `Attachment` (боевое оружие имеет визуал)
- **Если есть `Attachment`** → НЕ обязательно есть `WeaponStats` (коробка, кастрюля = мирные предметы)

**Код:**
```rust
#[derive(Component, Clone, Debug, Reflect)]
#[require(Attachment)]  // ✅ WeaponStats требует Attachment
pub struct WeaponStats { /* ... */ }
```

**Примеры spawn:**
```rust
// Меч (боевое оружие)
commands.spawn((
    Actor::default(),
    WeaponStats::melee_sword(),
    Attachment::weapon("res://weapons/sword.tscn"),
));

// Коробка (мирный предмет, НЕТ WeaponStats)
commands.spawn((
    Actor::default(),
    Attachment::item("res://items/crate.tscn"),
));
```

---

### Решение 3: Единый WeaponStats с WeaponType enum ✅

**Альтернативы:**
- **Вариант A:** Единый `WeaponStats` с `WeaponType` enum (выбрано)
- **Вариант B:** Отдельные `MeleeWeapon` + `RangedWeapon` компоненты

**Обоснование выбора A (KISS principle):**
- ✅ Простота кода (одна система cooldown)
- ✅ Hybrid оружие работает из коробки
- ✅ Меньше дублирования систем

**Trade-offs:**
- ⚠️ Unused поля (melee не использует `range`, ranged не использует `windup_duration`)
- **Решение:** Acceptable (memory footprint минимален, несколько f32 полей)

---

## Фаза 2.0: Weapon Architecture Refactoring

**Срок:** 1-2 дня
**Статус:** ⏸️ Planned
**Цель:** Перейти от `Attacker + Weapon` к `WeaponStats`

### Задачи

- [ ] **2.0.1 Создать `weapon_stats.rs`:**
  - [ ] `WeaponStats` component
  - [ ] `WeaponType` enum
  - [ ] Helper methods (`melee_sword()`, `ranged_pistol()`)
  - [ ] `can_attack()`, `start_cooldown()`, `is_melee()`, `is_ranged()`

- [ ] **2.0.2 Обновить `combat/mod.rs`:**
  - [ ] Удалить re-export `Attacker`
  - [ ] Добавить re-export `WeaponStats`
  - [ ] Обновить `CombatPlugin` системы

- [ ] **2.0.3 Рефакторинг ranged systems:**
  - [ ] `ai_weapon_fire_intent`: `Query<&Weapon>` → `Query<&WeaponStats>`
  - [ ] `update_weapon_cooldowns`: использовать `WeaponStats.cooldown_timer`
  - [ ] `process_weapon_fire_intents_main_thread`: использовать `WeaponStats.range`
  - [ ] `weapon_fire_main_thread`: использовать `WeaponStats.projectile_speed`

- [ ] **2.0.4 Обновить spawn code:**
  - [ ] `simulation_bridge.rs`: `delayed_npc_spawn_system` → `WeaponStats`
  - [ ] Удалить старые `Attacker` + `Weapon` spawns

- [ ] **2.0.5 Удалить старые файлы:**
  - [ ] `combat/attacker.rs` (полностью удалить)
  - [ ] `combat/weapon.rs`: удалить `Weapon` struct, оставить events
  - [ ] `components/combat.rs`: удалить дубликат `Attacker` struct

- [ ] **2.0.6 Тесты:**
  - [ ] `cargo test` проходит
  - [ ] Godot runtime: 2 NPC стреляют друг в друга (ranged работает)
  - [ ] Нет ошибок компиляции

### Что удаляем/заменяем

**Удалить:**
- ❌ `combat/attacker.rs` (весь файл)
- ❌ `Weapon` struct в `combat/weapon.rs`
- ❌ `Attacker` struct в `components/combat.rs`

**Оставить:**
- ✅ Events: `WeaponFired`, `ProjectileHit`, `WeaponFireIntent`
- ✅ `Attachment` component (без изменений)
- ✅ Godot systems (weapon_aim, weapon_fire)

**Заменить:**
- `Query<&Attacker>` → `Query<&WeaponStats>`
- `Query<&Weapon>` → `Query<&WeaponStats>`
- `attacker.base_damage` → `weapon_stats.base_damage`
- `weapon.range` → `weapon_stats.range`

### Тесты валидации

**Критерии успеха:**
- ✅ Ranged combat работает (AI стреляет, projectiles летят, урон наносится)
- ✅ Нет ошибок компиляции
- ✅ Нет ошибок в логах (GodotLogger)
- ✅ `cargo test` проходит (все существующие тесты)

---

## Фаза 2.1: Melee Combat Core

**Срок:** 3-4 дня
**Статус:** ⏸️ Planned
**Цель:** Базовая melee атака работает (windup → attack → recovery)

### 2.1.1 ECS Components

**Новые компоненты:**

```rust
// === MeleeAttackState: отслеживание фаз атаки ===
#[derive(Component, Clone, Debug, Reflect)]
pub struct MeleeAttackState {
    pub phase: AttackPhase,
    pub phase_timer: f32,
    pub target: Entity,
}

pub enum AttackPhase {
    Idle,
    Windup { duration: f32 },     // Замах (видимо для противника)
    Active { duration: f32 },      // Удар (hitbox enabled)
    Recovery { duration: f32 },    // Восстановление (vulnerable)
}
```

**Задачи:**
- [ ] Создать `combat/melee.rs`
- [ ] `MeleeAttackState` component
- [ ] `AttackPhase` enum
- [ ] Helper methods (`start_windup()`, `is_active()`, `advance_phase()`)

---

### 2.1.2 ECS Events

**Новые events:**

```rust
// === MeleeAttackIntent: AI хочет атаковать (ECS strategic) ===
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackIntent {
    pub attacker: Entity,
    pub target: Entity,
    pub attack_type: MeleeAttackType,
}

pub enum MeleeAttackType {
    Normal,  // Базовая атака
    Heavy,   // Медленно, но сильно (для будущего)
    Quick,   // Быстро, но слабо (для будущего)
}

// === MeleeAttackStarted: атака одобрена Godot (tactical validation passed) ===
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackStarted {
    pub attacker: Entity,
    pub target: Entity,
    pub attack_type: MeleeAttackType,
    pub windup_duration: f32,
    pub attack_duration: f32,
    pub recovery_duration: f32,
}

// === MeleeHit: hitbox collision detected (Godot → ECS) ===
#[derive(Event, Clone, Debug)]
pub struct MeleeHit {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: u32,
    pub was_blocked: bool,   // Цель блокировала удар
    pub was_parried: bool,   // Цель парировала
}
```

**Задачи:**
- [ ] Добавить events в `combat/melee.rs`
- [ ] Зарегистрировать в `CombatPlugin`
- [ ] Создать static queue для `MeleeHit` (Godot → ECS)

---

### 2.1.3 ECS Systems (Strategic Layer)

**Новые системы:**

```rust
// === ai_melee_attack_intent: генерирует intent когда AI близко к target ===
pub fn ai_melee_attack_intent(
    actors: Query<(Entity, &AIState, &WeaponStats)>,
    positions: Query<&StrategicPosition>,
    mut intent_events: EventWriter<MeleeAttackIntent>,
) {
    // Для каждого актёра в Combat state:
    // 1. Проверить weapon_stats.is_melee()
    // 2. Проверить weapon_stats.can_attack() (cooldown)
    // 3. Проверить distance < weapon_stats.attack_radius (strategic estimate)
    // 4. Генерировать MeleeAttackIntent
}

// === update_melee_attack_phases: обновляет фазы атаки ===
pub fn update_melee_attack_phases(
    mut query: Query<&mut MeleeAttackState>,
    time: Res<Time<Fixed>>,
) {
    // Для каждой активной атаки:
    // 1. Уменьшить phase_timer
    // 2. Если timer <= 0 → переход в следующую фазу
    // 3. Windup → Active → Recovery → Idle
}

// === start_melee_attacks: обрабатывает MeleeAttackStarted ===
pub fn start_melee_attacks(
    mut started_events: EventReader<MeleeAttackStarted>,
    mut commands: Commands,
    mut weapons: Query<&mut WeaponStats>,
) {
    // Для каждого события:
    // 1. Добавить MeleeAttackState (phase = Windup)
    // 2. Запустить cooldown (weapon_stats.start_cooldown())
}

// === update_weapon_cooldowns: обновляет cooldown таймеры ===
// (Уже есть, просто обновим для WeaponStats)
pub fn update_weapon_cooldowns(
    mut weapons: Query<&mut WeaponStats>,
    time: Res<Time<Fixed>>,
) {
    // Уменьшать cooldown_timer для всех оружий
}
```

**Задачи:**
- [ ] `ai_melee_attack_intent` система
- [ ] `update_melee_attack_phases` система
- [ ] `start_melee_attacks` система
- [ ] Обновить `update_weapon_cooldowns` для `WeaponStats`
- [ ] Зарегистрировать в `CombatPlugin` (правильный порядок!)

---

### 2.1.4 Godot Systems (Tactical Layer)

**Новые системы:**

```rust
// === process_melee_attack_intents_main_thread: tactical validation ===
pub fn process_melee_attack_intents_main_thread(
    mut intent_events: EventReader<MeleeAttackIntent>,
    visuals: NonSend<VisualRegistry>,
    weapons: Query<&WeaponStats>,
    mut started_events: EventWriter<MeleeAttackStarted>,
) {
    // Для каждого intent:
    // 1. Получить Godot Transform (shooter + target)
    // 2. Проверить distance < weapon_stats.attack_radius
    // 3. (Optional) Проверить line of sight
    // 4. Если OK → генерировать MeleeAttackStarted
}

// === execute_melee_attacks_main_thread: animation + hitbox ===
pub fn execute_melee_attacks_main_thread(
    query: Query<(Entity, &MeleeAttackState), Changed<MeleeAttackState>>,
    visuals: NonSend<VisualRegistry>,
    attachments: NonSend<AttachmentRegistry>,
) {
    // Для каждой атаки с изменённой фазой:
    // 1. Phase = Windup → trigger animation "attack_windup"
    // 2. Phase = Active → enable weapon hitbox (Area3D.monitoring = true)
    // 3. Phase = Recovery → disable hitbox (Area3D.monitoring = false)
    // 4. Phase = Idle → (ничего не делаем)
}

// === process_melee_hits: читает MeleeHit queue → DamageDealt events ===
pub fn process_melee_hits(
    mut hit_queue: ResMut<MeleeHitQueue>,
    targets: Query<&mut Health>,
    weapons: Query<&WeaponStats>,
    mut damage_events: EventWriter<DamageDealt>,
) {
    // Для каждого MeleeHit из queue:
    // 1. Проверить self-hit (attacker == target → skip)
    // 2. Рассчитать damage (weapon_stats.base_damage × modifiers)
    // 3. Применить damage reduction (если blocked/parried)
    // 4. Нанести урон (health.take_damage())
    // 5. Генерировать DamageDealt event
}
```

**Задачи:**
- [ ] Создать `voidrun_godot/src/systems/melee_system.rs`
- [ ] `process_melee_attack_intents_main_thread`
- [ ] `execute_melee_attacks_main_thread`
- [ ] `process_melee_hits`
- [ ] Зарегистрировать в `simulation_bridge.rs`
- [ ] Создать `MeleeHitQueue` (static queue, как ProjectileHitQueue)

---

### 2.1.5 TSCN Prefabs

**test_sword.tscn:**

```gdscene
[node name="Sword" type="Node3D"]

[node name="WeaponPlacement" type="Node3D" parent="."]
transform = Transform3D(...)  # Правильная ориентация в руке

[node name="Mesh" type="MeshInstance3D" parent="WeaponPlacement"]
mesh = SubResource("...")  # Визуал меча

[node name="Hitbox" type="Area3D" parent="WeaponPlacement"]
collision_layer = 8   # Melee weapons layer
collision_mask = 2    # Actors layer
monitoring = false    # Disabled до Active phase

[node name="HitboxShape" type="CollisionShape3D" parent="WeaponPlacement/Hitbox"]
shape = SubResource("CapsuleShape3D")  # Форма hitbox (вдоль лезвия)
```

**Задачи:**
- [ ] Создать `godot/weapons/test_sword.tscn`
- [ ] Mesh для визуала (простая капсула/box)
- [ ] Area3D hitbox (disabled by default)
- [ ] CapsuleShape3D вдоль лезвия (~1.5м длина)
- [ ] Signal connection: `hitbox.body_entered` → Rust callback

---

### 2.1.6 Integration & Testing

**Spawn test actors:**

```rust
// В delayed_npc_spawn_system:
commands.spawn((
    Actor { faction_id: 1 },
    WeaponStats::melee_sword(),
    Attachment::weapon("res://weapons/test_sword.tscn"),
    AIState::Combat { target: npc2 },
    // ...
));
```

**Задачи:**
- [ ] Обновить `delayed_npc_spawn_system` (spawнить 2 NPC с мечами)
- [ ] Smoke test: 2 NPC сходятся и атакуют друг друга
- [ ] Проверить фазы: Windup → Active → Recovery
- [ ] Проверить hitbox collision (логи MeleeHit)
- [ ] Проверить урон (health уменьшается)

**Критерии успеха:**
- ✅ 2 NPC с мечами атакуют друг друга
- ✅ Видны фазы атаки (windup → active → recovery)
- ✅ Hitbox collision работает (MeleeHit events генерируются)
- ✅ Урон наносится (health уменьшается, DamageDealt events)
- ✅ Cooldown работает (не спамят атаки)

---

## Фаза 2.2: Defensive Mechanics

**Срок:** 2-3 дня
**Статус:** ⏸️ Planned
**Цель:** Блок, парирование, уклонение работают

### 2.2.1 Block System

**Компонент:**

```rust
#[derive(Component, Clone, Debug, Reflect)]
pub struct BlockState {
    pub is_blocking: bool,
    pub block_stamina_cost_per_sec: f32,  // 5.0
}
```

**Механика:**
- Держать block → постоянный расход stamina (5/sec)
- Блокированная атака → урон × 0.3 (70% reduction)
- Если stamina закончилась → block broken

**Системы:**

```rust
// === consume_block_stamina: расход stamina при блоке ===
pub fn consume_block_stamina(
    mut query: Query<(&mut Stamina, &BlockState)>,
    time: Res<Time<Fixed>>,
) {
    // Для каждого блокирующего:
    // 1. Проверить is_blocking
    // 2. Расходовать stamina (cost_per_sec × delta_time)
    // 3. Если stamina < 0 → снять BlockState
}

// === apply_block_reduction: уменьшение урона при блоке ===
// (Интегрируется в process_melee_hits)
if target_has_block_state && target_is_blocking {
    final_damage *= 0.3;
    melee_hit.was_blocked = true;
}
```

**Задачи:**
- [ ] `BlockState` component
- [ ] `consume_block_stamina` система
- [ ] Интеграция в `process_melee_hits` (damage reduction)
- [ ] AI: блокировать когда HP < 50% и stamina > 30

---

### 2.2.2 Parry System

**Компонент:**

```rust
#[derive(Component, Clone, Debug, Reflect)]
pub struct ParryState {
    pub parry_window_active: bool,
    pub parry_window_timer: f32,
    pub parry_window_duration: f32,  // 0.15s
}
```

**Механика:**
- Parry активируется на короткое окно (0.15s)
- Если melee hit попадает в окно → 100% блок + stagger противника
- Stagger = противник не может атаковать 0.5s (cooldown принудительно)
- Stamina cost 15 (единожды)

**Системы:**

```rust
// === update_parry_window: обновляет таймер окна ===
pub fn update_parry_window(
    mut query: Query<&mut ParryState>,
    time: Res<Time<Fixed>>,
) {
    // Для каждого парирующего:
    // 1. Уменьшать parry_window_timer
    // 2. Если timer <= 0 → parry_window_active = false
}

// === apply_parry_effects: stagger + 100% блок ===
// (Интегрируется в process_melee_hits)
if target_has_parry_state && target_parry_window_active {
    final_damage = 0;  // 100% блок
    melee_hit.was_parried = true;

    // Stagger attacker
    if let Ok(mut weapon) = weapons.get_mut(attacker) {
        weapon.cooldown_timer += 0.5;  // +0.5s cooldown
    }
}
```

**Задачи:**
- [ ] `ParryState` component
- [ ] `update_parry_window` система
- [ ] Интеграция в `process_melee_hits` (100% блок + stagger)
- [ ] AI: парировать когда видит Windup phase (60% accuracy)

---

### 2.2.3 Dodge System

**Компонент:**

```rust
#[derive(Component, Clone, Debug, Reflect)]
pub struct DodgeState {
    pub is_dodging: bool,
    pub iframe_timer: f32,
    pub iframe_duration: f32,  // 0.2s
    pub dodge_direction: Vec3,  // Направление dash'а
}
```

**Механика:**
- Dodge активируется → i-frames 0.2s (invulnerability)
- Во время i-frames → все входящие атаки игнорируются
- Dash движение в сторону (2 метра)
- Stamina cost 25 (единожды)

**Системы:**

```rust
// === update_dodge_iframes: обновляет i-frames таймер ===
pub fn update_dodge_iframes(
    mut query: Query<&mut DodgeState>,
    time: Res<Time<Fixed>>,
) {
    // Для каждого уклоняющегося:
    // 1. Уменьшать iframe_timer
    // 2. Если timer <= 0 → is_dodging = false
}

// === apply_dodge_movement: dash движение ===
pub fn apply_dodge_movement(
    query: Query<(Entity, &DodgeState), Added<DodgeState>>,
    mut commands: Commands,
) {
    // Для каждого начавшего dodge:
    // 1. Добавить MovementCommand (dash в dodge_direction)
    // 2. Override текущее движение
}

// === apply_dodge_invulnerability: игнор урона ===
// (Интегрируется в process_melee_hits)
if target_has_dodge_state && target_is_dodging && iframe_timer > 0 {
    continue;  // Пропустить урон полностью
}
```

**Задачи:**
- [ ] `DodgeState` component
- [ ] `update_dodge_iframes` система
- [ ] `apply_dodge_movement` система (dash)
- [ ] Интеграция в `process_melee_hits` (invulnerability)
- [ ] AI: уклоняться когда stamina < 20 (50% accuracy)

---

### 2.2.4 Testing

**Критерии успеха:**
- ✅ Block работает: урон × 0.3, stamina drain 5/sec
- ✅ Parry работает: 100% блок, stagger 0.5s, timing window 0.15s
- ✅ Dodge работает: i-frames 0.2s, dash движение, stamina cost 25
- ✅ AI использует все 3 defensive options (не застревает в одной)

---

## Фаза 2.3: AI Melee Combat

**Срок:** 2-3 дня
**Статус:** ⏸️ Planned
**Цель:** AI разумно принимает решения (parry/dodge/block/counterattack)

### 2.3.1 AI FSM Extension

**Расширение AIState:**

```rust
pub enum AIState {
    Idle,
    Patrol { waypoint: Vec3 },
    Combat {
        target: Entity,
        combat_stance: CombatStance,
        last_defensive_action: Option<(DefensiveAction, f32)>,  // (action, timestamp)
    },
    Retreat { from: Vec3 },
    Dead,
}

pub enum CombatStance {
    Aggressive,  // Больше атак, меньше блоков
    Defensive,   // Больше блоков/parry
    Balanced,    // 50/50
}

pub enum DefensiveAction {
    Block,
    Parry,
    Dodge,
}
```

**Задачи:**
- [ ] Обновить `AIState::Combat` (добавить stance, last_action)
- [ ] `CombatStance` enum
- [ ] `DefensiveAction` enum

---

### 2.3.2 AI Decision Making

**Новая система:**

```rust
// === ai_defensive_decision: принимает решение о защите ===
pub fn ai_defensive_decision(
    mut actors: Query<(Entity, &mut AIState, &Stamina)>,
    enemy_attacks: Query<(Entity, &MeleeAttackState)>,
    mut commands: Commands,
) {
    // Для каждого актёра в Combat:
    // 1. Проверить: атакует ли target? (query enemy MeleeAttackState)
    // 2. Если phase = Windup:
    //    - Stamina > 50 && can_parry → 60% chance Parry
    //    - Stamina < 30 → 70% chance Dodge
    //    - Default → Block
    // 3. Добавить соответствующий компонент (ParryState/DodgeState/BlockState)
}

// === ai_counterattack_opportunity: counterattack после parry ===
pub fn ai_counterattack_opportunity(
    actors: Query<(Entity, &AIState)>,
    enemy_attacks: Query<(Entity, &MeleeAttackState)>,
    mut intent_events: EventWriter<MeleeAttackIntent>,
) {
    // Для каждого актёра:
    // 1. Проверить: target в Recovery phase?
    // 2. Если да → immediate MeleeAttackIntent (counterattack window)
}
```

**Задачи:**
- [ ] `ai_defensive_decision` система (parry/dodge/block choice)
- [ ] `ai_counterattack_opportunity` система
- [ ] Tuning вероятностей (parry 60%, dodge 50%)
- [ ] Cooldown на defensive actions (не спамить parry каждый frame)

---

### 2.3.3 Windup Detection

**Логика:**

AI должен видеть `MeleeAttackState.phase = Windup` у противника и реагировать.

```rust
// В ai_defensive_decision:
for (entity, ai_state, stamina) in actors.iter_mut() {
    let AIState::Combat { target, .. } = ai_state else { continue };

    // Проверяем: target атакует?
    if let Ok((_, enemy_attack)) = enemy_attacks.get(*target) {
        if matches!(enemy_attack.phase, AttackPhase::Windup { .. }) {
            // Противник замахивается → принять решение о защите
            decide_defensive_action(entity, stamina, &mut commands);
        }
    }
}
```

**Задачи:**
- [ ] Query enemy `MeleeAttackState` в AI системе
- [ ] Реакция на Windup phase (decision window)
- [ ] Логирование решений (debug: "NPC парирует атаку")

---

### 2.3.4 Testing

**Критерии успеха:**
- ✅ AI видит Windup противника
- ✅ AI принимает разумные решения (parry когда stamina хорошая, dodge когда плохая)
- ✅ AI использует counterattack после успешного parry
- ✅ AI не застревает (не спамит одну defensive option)
- ✅ Combat динамичный (не бесконечный block standoff)

---

## Фаза 2.4: Polish & Balance

**Срок:** 1-2 дня
**Статус:** ⏸️ Planned
**Цель:** Боевая система чувствуется хорошо

### 2.4.1 Animations

**Godot AnimationPlayer:**

```gdscene
[node name="AnimationPlayer" type="AnimationPlayer" parent="."]

[animation name="attack_windup"]
# Замах меча (0.2s)

[animation name="attack_strike"]
# Удар меча (0.1s)

[animation name="attack_recovery"]
# Возврат в стойку (0.2s)

[animation name="block_stance"]
# Блок стойка (loop)

[animation name="parry_flash"]
# Parry вспышка (0.1s)

[animation name="dodge_dash"]
# Dash анимация (0.2s)
```

**Задачи:**
- [ ] Создать animations в Godot (простые, placeholder)
- [ ] Интегрировать в `execute_melee_attacks_main_thread`
- [ ] Trigger animations по фазам (Windup → "attack_windup")

---

### 2.4.2 VFX

**Visual effects:**

- **Parry flash:** Синяя вспышка при успешном парировании
- **Dodge dash:** След (motion blur) при dash'е
- **Block impact:** Красные искры при блокированном ударе
- **Stagger:** Красная outline у stagger'нутого

**Задачи:**
- [ ] CPUParticles3D для parry flash (синие частицы)
- [ ] Trail effect для dodge dash
- [ ] Impact particles для block
- [ ] Visual feedback для stagger (shader/outline)

---

### 2.4.3 Balancing

**Stamina costs:**
- Block: 5/sec (можем изменить на 3-7)
- Parry: 15 (можем изменить на 10-20)
- Dodge: 25 (можем изменить на 20-30)
- Attack: 30 (можем изменить на 25-35)

**Timings:**
- Windup: 0.2s (можем изменить на 0.15-0.3s)
- Active: 0.1s (можем изменить на 0.08-0.15s)
- Recovery: 0.2s (можем изменить на 0.15-0.3s)
- Parry window: 0.15s (можем изменить на 0.1-0.2s)
- I-frames: 0.2s (можем изменить на 0.15-0.25s)

**AI probabilities:**
- Parry: 60% (можем изменить на 40-80%)
- Dodge: 50% (можем изменить на 30-70%)
- Block: fallback

**Задачи:**
- [ ] Playtesting (запустить 10 боёв NPC vs NPC)
- [ ] Tuning stamina costs (если слишком быстро кончается)
- [ ] Tuning timings (если слишком быстро/медленно)
- [ ] Tuning AI probabilities (если AI слишком тупой/умный)

---

### 2.4.4 Final Testing

**Критерии успеха:**
- ✅ 2 NPC дерутся друг с другом 30+ секунд
- ✅ Используют все defensive options (block/parry/dodge)
- ✅ Combat выглядит динамично (не застревают)
- ✅ Animations/VFX работают
- ✅ Stamina balance адекватен (не кончается за 2 секунды)
- ✅ Можно играть 5 минут без скуки

---

## Риски и митигация

### Риск 1: Hitbox collision detection нестабильна

**Описание:** Area3D hitbox иногда не детектит collision (Godot physics bug).

**Вероятность:** Средняя
**Влияние:** Высокое (core mechanic не работает)

**Митигация:**
- Увеличить hitbox размер (более generous collision)
- Raycast fallback (если Area3D не сработал)
- Debug визуализация hitbox (видеть что происходит)

---

### Риск 2: AI застревает в бесконечном блоке

**Описание:** Оба NPC блокируют → никто не атакует → standoff.

**Вероятность:** Средняя
**Влияние:** Среднее (combat скучный)

**Митигация:**
- Cooldown на block (не держать > 2 секунд)
- Forced aggression (если оба блокируют > 3 сек → один атакует)
- CombatStance rotation (Balanced → Aggressive после 5 сек)

---

### Риск 3: Stamina balance слишком жёсткий

**Описание:** Stamina кончается за 2-3 атаки → combat превращается в exhaustion fest.

**Вероятность:** Средняя
**Влияние:** Среднее (gameplay frustrating)

**Митигация:**
- Playtesting (смотреть средняя длительность боя)
- Увеличить max stamina (100 → 150)
- Увеличить regen rate (10 → 15)
- Уменьшить costs (attack 30 → 25)

---

### Риск 4: Refactoring ломает ranged combat

**Описание:** Переход `Weapon` → `WeaponStats` вносит баги в существующий ranged code.

**Вероятность:** Низкая
**Влияние:** Среднее (откатываем изменения)

**Митигация:**
- Тестировать ranged после каждого шага рефакторинга
- Git commits после каждой подфазы (легко откатиться)
- Smoke test: 2 NPC стреляют друг в друга (перед melee)

---

## Tracking

### Current Phase

- [ ] **Фаза 2.0:** Weapon Architecture Refactoring
- [ ] **Фаза 2.1:** Melee Combat Core
- [ ] **Фаза 2.2:** Defensive Mechanics
- [ ] **Фаза 2.3:** AI Melee Combat
- [ ] **Фаза 2.4:** Polish & Balance

### Blocked Issues

(Пусто пока)

### Completed Milestones

- ✅ **Фаза 1:** Архитектурные решения (2025-01-13)

---

## Связанные документы

**Architecture Decision Records:**
- [ADR-003: ECS vs Godot Physics Ownership](../decisions/ADR-003-ecs-vs-godot-physics-ownership.md)
- [ADR-004: Command/Event Architecture](../decisions/ADR-004-command-event-architecture.md)
- [ADR-007: TSCN Prefabs + Dynamic Attachment](../decisions/ADR-007-tscn-prefabs-dynamic-attachment.md)

**Design Docs:**
- [Shield Technology](../design/shield-technology.md) — Почему melee + ranged сосуществуют

**Roadmap:**
- [Roadmap - Фаза 1.5](../roadmap.md#фаза-15-combat-mechanics-текущее)

**Related Systems:**
- `combat/weapon.rs` — Ranged weapon system (events)
- `ai/simple_fsm.rs` — AI FSM (будет расширен)
- `components/attachment.rs` — Attachment system (без изменений)

---

## Changelog

**2025-01-13:** Создан документ, утверждены архитектурные решения (Фаза 1).

---

**Следующий шаг:** Начать Фазу 2.0 (Weapon Architecture Refactoring).
