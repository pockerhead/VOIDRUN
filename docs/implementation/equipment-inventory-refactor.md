# Equipment & Inventory System Refactor

**Статус:** Phase 1-3 Complete ✅ | Phase 4-7 Postponed ⏸️
**Версия:** 1.1
**Дата:** 2025-10-19 (updated)

---

## Обзор

Полный рефакторинг системы экипировки и инвентаря для поддержки:
- ✅ **Weapons (1-4)** - Large/Small слоты с smooth switching
- ✅ **Consumables (5-9)** - Hotbar с динамическим unlock через броню
- ✅ **Armor** - Визуал + stats + consumable slot bonus
- ✅ **Energy Shield** - Пассивный барьер (блокирует ranged, из диздока)
- ✅ **Inventory** - General storage (weight/volume limits позже)

**КРИТИЧЕСКИ ВАЖНО:** Вся система работает для **Player И AI actors** одинаково!
- AI переключают оружие (tactical decisions)
- AI используют consumables (аптечки в бою)
- AI имеют разную броню (влияет на consumable slots)
- AI имеют энергощиты (разные модели по faction)

---

## Текущие проблемы

### Проблема 1: Смешение Equipment и Hotbar
```rust
// СЕЙЧАС: Inventory.slots[9] - что это? Оружие или аптечки?
struct Inventory {
    slots: [Option<ItemStack>; 9], // ❌ Непонятно
}
```

### Проблема 2: Дублирование данных
```rust
// ItemStack хранит компоненты напрямую
ItemStack::Weapon {
    stats: WeaponStats,      // ← Дублируется
    attachment: Attachment   // ← Дублируется
}

// Entity тоже имеет WeaponStats + Attachment
// Данные копируются туда-сюда при weapon switch
```

### Проблема 3: Detach не работает
- `Changed<Attachment>` триггерится только при MODIFY, не при REMOVE
- `DetachAttachment` marker компонент не используется
- Visual swap сломан

### Проблема 4: Неконсистентная архитектура
- Equipment data живёт в Inventory (ItemStack)
- Active weapon data живёт на entity (WeaponStats, Attachment)
- Weapon switch КОПИРУЕТ данные между ними
- Нет чёткого lifecycle (equip/unequip)

---

## Архитектура (новая)

### 1. Item Data Model

**ItemDefinition** - статические данные (blueprint):
```rust
#[derive(Clone, Debug)]
struct ItemDefinition {
    id: ItemId,
    name: String,
    item_type: ItemType,

    // Weapon-specific
    weapon_template: Option<WeaponStatsTemplate>,
    prefab_path: Option<String>,
    attachment_point: Option<String>,

    // Armor-specific
    armor_stats: Option<ArmorStatsTemplate>,

    // Consumable-specific
    consumable_effect: Option<ConsumableEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ItemId(String); // "melee_sword", "pistol_basic", "health_kit", etc.

enum ItemType {
    Weapon { size: WeaponSize },
    Armor,
    Shield, // Физический щит (не энергобарьер!)
    Consumable,
    CraftMaterial,
    Quest,
}

enum WeaponSize {
    Large,  // Винтовка, меч, кувалда
    Small,  // Пистолет, кинжал
}
```

**ItemInstance** - runtime данные (конкретный предмет):
```rust
#[derive(Clone, Debug, Reflect)]
struct ItemInstance {
    definition_id: ItemId,
    stack_size: u32,           // Для stackable (consumables, materials)
    durability: Option<f32>,   // 0.0-1.0 для оружия/брони
    ammo_count: Option<u32>,   // Для ranged weapons
}
```

**ItemDefinitions** - lookup table (resource):
```rust
#[derive(Resource)]
struct ItemDefinitions {
    definitions: HashMap<ItemId, ItemDefinition>,
}

impl ItemDefinitions {
    fn get(&self, id: &ItemId) -> Option<&ItemDefinition> {
        self.definitions.get(id)
    }
}
```

---

### 2. Equipment Components

**EquippedWeapons** - что держит в руках (hotkeys 1-4):
```rust
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct EquippedWeapons {
    // Weapon slots
    primary_large_1: Option<EquippedItem>,  // [1]
    primary_large_2: Option<EquippedItem>,  // [2]
    secondary_small_1: Option<EquippedItem>, // [3]
    secondary_small_2: Option<EquippedItem>, // [4]

    // Active slot (0-3 какой сейчас в руках)
    active_slot: u8,
}

#[derive(Clone, Debug, Reflect)]
struct EquippedItem {
    definition_id: ItemId,
    durability: f32,         // Runtime state
    ammo_count: Option<u32>, // Runtime ammo
}

// NOTE: При equip добавляем WeaponStats + Attachment компоненты на entity
//       При unequip удаляем их
```

**ConsumableSlots** - быстрый доступ (hotkeys 5-9):
```rust
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct ConsumableSlots {
    slots: [Option<ItemInstance>; 5], // [5-9]
    unlocked_count: u8, // 2-5 (базовые 2 + armor bonus)
}

impl ConsumableSlots {
    fn is_slot_unlocked(&self, index: u8) -> bool {
        index < self.unlocked_count
    }

    fn unlock_slots(&mut self, count: u8) {
        self.unlocked_count = count.min(5);
    }
}
```

**Armor** - пассивная защита + визуал:
```rust
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct Armor {
    definition_id: ItemId,
    durability: f32,

    // Stats
    defense: u32,
    consumable_slot_bonus: u8, // 0-3 доп слота
}

// При equip armor:
// 1. Добавляем Armor компонент
// 2. Добавляем Attachment (визуал) с prefab_path для "%Body"
// 3. Обновляем ConsumableSlots.unlocked_count = 2 + bonus
```

**EnergyShield** - энергобарьер (из shield-technology.md):
```rust
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct EnergyShield {
    max_energy: f32,
    current_energy: f32,
    recharge_rate: f32,      // энергия/сек (вне боя)
    recharge_delay: f32,     // сколько ждать после урона
    velocity_threshold: f32, // 5.0 м/с (kinetic threshold)
}

// Всегда блокирует только ranged (velocity > threshold)
// Melee полностью игнорирует щит (slow attacks pass through)
```

**Inventory** - общая свалка (UI grid):
```rust
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct Inventory {
    items: Vec<ItemInstance>,
    capacity: usize, // Пока unlimited, позже weight/volume
}
```

---

### 3. Hotkey Layout

```
[1] Primary Large 1  ─┐
[2] Primary Large 2   ├─ Weapon switching (smooth holster/draw)
[3] Secondary Small 1 │
[4] Secondary Small 2 ─┘

[5] Consumable 1 (карман)  ─┐
[6] Consumable 2 (карман)   ├─ Instant use
[7] Consumable 3 (подсумок) │  (если разблокирован броней)
[8] Consumable 4 (подсумок) │
[9] Consumable 5 (подсумок) ─┘

Armor:  Пассивный (визуал + defense + unlock slots 7-9)
Shield: Пассивный (энергобарьер HP + recharge)
```

---

## Equipment Lifecycle

### Events

```rust
// ===== Weapon management =====

/// Equip weapon в конкретный слот
#[derive(Event)]
struct EquipWeaponIntent {
    entity: Entity,
    slot: WeaponSlot,
    item: ItemInstance,
}

/// Unequip weapon из слота
#[derive(Event)]
struct UnequipWeaponIntent {
    entity: Entity,
    slot: WeaponSlot,
}

/// Swap активного оружия (hotkeys 1-4)
#[derive(Event)]
struct SwapActiveWeaponIntent {
    entity: Entity,
    target_slot: u8, // 0-3
}

enum WeaponSlot {
    PrimaryLarge1,
    PrimaryLarge2,
    SecondarySmall1,
    SecondarySmall2,
}

// ===== Armor/Shield =====

/// Equip armor
#[derive(Event)]
struct EquipArmorIntent {
    entity: Entity,
    item: ItemInstance,
}

// Shield - пассивный компонент, всегда активен (no equip event)

// ===== Consumables =====

/// Use consumable (hotkeys 5-9)
#[derive(Event)]
struct UseConsumableIntent {
    entity: Entity,
    slot_index: u8, // 0-4 (соответствует hotkeys 5-9)
}
```

---

### Systems

#### Equip Weapon

```rust
fn process_equip_weapon(
    mut commands: Commands,
    mut events: EventReader<EquipWeaponIntent>,
    mut equipped: Query<&mut EquippedWeapons>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok(mut weapons) = equipped.get_mut(intent.entity) else {
            continue;
        };

        // 1. Unequip старое оружие (если есть)
        if let Some(old_item) = weapons.get_slot_mut(intent.slot) {
            // Remove WeaponStats + Attachment components
            commands.entity(intent.entity)
                .remove::<WeaponStats>()
                .remove::<Attachment>();

            // TODO: вернуть old_item в Inventory
        }

        // 2. Equip новое оружие
        weapons.set_slot(intent.slot, Some(EquippedItem {
            definition_id: intent.item.definition_id.clone(),
            durability: intent.item.durability.unwrap_or(1.0),
            ammo_count: intent.item.ammo_count,
        }));

        // 3. Добавить WeaponStats + Attachment components
        let Some(def) = definitions.get(&intent.item.definition_id) else {
            log_error(&format!("ItemDefinition not found: {:?}", intent.item.definition_id));
            continue;
        };

        if let Some(template) = &def.weapon_template {
            commands.entity(intent.entity).insert((
                template.to_weapon_stats(),
                Attachment {
                    prefab_path: def.prefab_path.clone().unwrap(),
                    attachment_point: def.attachment_point.clone().unwrap(),
                    attachment_type: AttachmentType::Weapon,
                },
            ));
        }
    }
}
```

#### Weapon Swap (Smooth Transition)

```rust
fn process_weapon_swap(
    mut commands: Commands,
    mut events: EventReader<SwapActiveWeaponIntent>,
    mut equipped: Query<(&mut EquippedWeapons, &mut Attachment, &mut WeaponStats)>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok((mut weapons, mut attachment, mut weapon_stats)) =
            equipped.get_mut(intent.entity) else {
            continue;
        };

        // Guard: уже активен
        if weapons.active_slot == intent.target_slot {
            continue;
        }

        // Guard: слот пустой
        let Some(new_weapon) = weapons.get_slot(intent.target_slot) else {
            log("⚠️ Слот пустой");
            continue;
        };

        // === Smooth swap flow ===

        // 1. Start holster animation (TODO: animation event)
        // commands.entity(intent.entity).insert(HolsterAnimation { timer: 0.5 });

        // 2. Detach old weapon (empty prefab_path triggers Changed<Attachment>)
        attachment.prefab_path = "".into();

        // 3. Attach new weapon
        let Some(def) = definitions.get(&new_weapon.definition_id) else {
            continue;
        };

        attachment.prefab_path = def.prefab_path.clone().unwrap();
        attachment.attachment_point = def.attachment_point.clone().unwrap();

        // 4. Update WeaponStats
        if let Some(template) = &def.weapon_template {
            *weapon_stats = template.to_weapon_stats();
        }

        // 5. Start draw animation (TODO: animation event)
        // commands.entity(intent.entity).insert(DrawAnimation { timer: 0.3 });

        // 6. Update active slot
        weapons.active_slot = intent.target_slot;

        log(&format!("✅ Weapon swap → slot {} ({})",
            intent.target_slot, def.name));
    }
}
```

#### Use Consumable

```rust
fn process_use_consumable(
    mut events: EventReader<UseConsumableIntent>,
    mut consumables: Query<&mut ConsumableSlots>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok(mut slots) = consumables.get_mut(intent.entity) else {
            continue;
        };

        // Guard: слот разблокирован?
        if !slots.is_slot_unlocked(intent.slot_index) {
            log("⚠️ Слот заблокирован - нужна лучшая броня!");
            continue;
        }

        // Take consumable from slot
        let Some(item) = slots.slots[intent.slot_index as usize].take() else {
            log("⚠️ Слот пустой");
            continue;
        };

        // Get definition
        let Some(def) = definitions.get(&item.definition_id) else {
            continue;
        };

        // Apply consumable effect
        if let Some(effect) = &def.consumable_effect {
            apply_consumable_effect(intent.entity, effect);
            log(&format!("✅ Использован {} из слота {}",
                def.name, intent.slot_index + 5));
        }
    }
}
```

#### Equip Armor

```rust
fn process_equip_armor(
    mut commands: Commands,
    mut events: EventReader<EquipArmorIntent>,
    mut consumables: Query<&mut ConsumableSlots>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Some(def) = definitions.get(&intent.item.definition_id) else {
            continue;
        };

        let Some(armor_stats) = &def.armor_stats else {
            log_error("Item is not armor!");
            continue;
        };

        // 1. Add Armor component
        commands.entity(intent.entity).insert(Armor {
            definition_id: intent.item.definition_id.clone(),
            durability: intent.item.durability.unwrap_or(1.0),
            defense: armor_stats.defense,
            consumable_slot_bonus: armor_stats.consumable_slot_bonus,
        });

        // 2. Add Attachment (визуал)
        commands.entity(intent.entity).insert(Attachment {
            prefab_path: def.prefab_path.clone().unwrap(),
            attachment_point: "%Body".into(),
            attachment_type: AttachmentType::Armor,
        });

        // 3. Unlock consumable slots
        if let Ok(mut slots) = consumables.get_mut(intent.entity) {
            let unlocked = 2 + armor_stats.consumable_slot_bonus;
            slots.unlock_slots(unlocked);

            log(&format!("✅ Armor equipped - {} consumable slots unlocked", unlocked));
        }
    }
}
```

---

## Detach Fix

### Подход: Empty prefab_path

```rust
// В attach_prefabs_main_thread (существующая система):
fn attach_prefabs_main_thread(
    query: Query<(Entity, &Attachment), Changed<Attachment>>,
    visuals: NonSend<VisualRegistry>,
    mut attachments: NonSendMut<AttachmentRegistry>,
) {
    for (entity, attachment) in query.iter() {
        // NEW: Empty prefab_path → detach старый prefab
        if attachment.prefab_path.is_empty() {
            detach_existing_prefab(entity, &attachment.attachment_point, &mut attachments);
            log(&format!("🔄 Detached prefab from {} at {}",
                entity, attachment.attachment_point));
            continue;
        }

        // Attach новый prefab (как обычно)
        attach_single_prefab(entity, attachment, &visuals, &mut attachments);
    }
}

// Helper для detach
fn detach_existing_prefab(
    entity: Entity,
    attachment_point: &str,
    registry: &mut AttachmentRegistry,
) {
    let key = (entity, attachment_point.to_string());

    if let Some(mut node) = registry.attachments.remove(&key) {
        node.queue_free();
        log(&format!("🗑️ Removed attachment at {}", attachment_point));
    }
}
```

**Rationale:**
- ✅ Простой и консистентный с Changed<Attachment> паттерном
- ✅ Нет дополнительных marker components
- ✅ Работает для всех attachment types (weapons, armor)

---

## AI Integration

**КРИТИЧЕСКИ ВАЖНО:** Вся система работает для AI actors!

### AI Weapon Switching

```rust
// AI tactical decision system
fn ai_weapon_switch_decision(
    mut switch_events: EventWriter<SwapActiveWeaponIntent>,
    ai_query: Query<(Entity, &EquippedWeapons, &AIState, &CombatTarget)>,
) {
    for (entity, weapons, ai_state, target) in ai_query.iter() {
        // Тактическое решение на основе дистанции
        let distance_to_target = calculate_distance(entity, target.entity);

        if distance_to_target > 10.0 {
            // Переключиться на ranged weapon
            if weapons.has_ranged_weapon() && !weapons.is_ranged_active() {
                switch_events.write(SwapActiveWeaponIntent {
                    entity,
                    target_slot: weapons.find_ranged_slot().unwrap(),
                });
            }
        } else if distance_to_target < 3.0 {
            // Переключиться на melee weapon
            if weapons.has_melee_weapon() && !weapons.is_melee_active() {
                switch_events.write(SwapActiveWeaponIntent {
                    entity,
                    target_slot: weapons.find_melee_slot().unwrap(),
                });
            }
        }
    }
}
```

### AI Consumable Usage

```rust
// AI использует аптечки когда HP < 30%
fn ai_consumable_usage(
    mut use_events: EventWriter<UseConsumableIntent>,
    ai_query: Query<(Entity, &Health, &ConsumableSlots), With<AIState>>,
) {
    for (entity, health, consumables) in ai_query.iter() {
        if health.current as f32 / health.max as f32 < 0.3 {
            // Найти health kit в consumable slots
            if let Some(slot_index) = find_health_kit_slot(consumables) {
                use_events.write(UseConsumableIntent {
                    entity,
                    slot_index,
                });

                log(&format!("🤖 AI {:?} uses health kit", entity));
            }
        }
    }
}

fn find_health_kit_slot(consumables: &ConsumableSlots) -> Option<u8> {
    for (i, slot) in consumables.slots.iter().enumerate() {
        if let Some(item) = slot {
            if item.definition_id.0.contains("health_kit") {
                return Some(i as u8);
            }
        }
    }
    None
}
```

### AI Equipment Configuration

```rust
// При spawn AI - задать equipment на основе faction
fn spawn_ai_with_equipment(
    commands: &mut Commands,
    faction_id: u32,
    definitions: &ItemDefinitions,
) {
    let equipment = match faction_id {
        1 => {
            // Military faction - heavy armor + rifle
            (
                vec![ItemInstance::new("rifle_basic"), ItemInstance::new("pistol_basic")],
                ItemInstance::new("armor_military"),
                true, // has energy shield
            )
        }
        2 => {
            // Melee cult - no shield, heavy melee
            (
                vec![ItemInstance::new("melee_sword"), ItemInstance::new("dagger")],
                ItemInstance::new("armor_light"),
                false, // no shield
            )
        }
        _ => {
            // Default raiders
            (
                vec![ItemInstance::new("pistol_basic")],
                ItemInstance::new("armor_scrap"),
                false,
            )
        }
    };

    commands.spawn((
        Actor { faction_id },
        AIState::default(),
        EquippedWeapons::with_items(equipment.0),
        ConsumableSlots::default(),
        Inventory::empty(),
    ));

    // Equip armor
    // TODO: emit EquipArmorIntent

    // Add shield if faction has it
    if equipment.2 {
        commands.insert(EnergyShield {
            max_energy: 500.0,
            current_energy: 500.0,
            recharge_rate: 20.0,
            recharge_delay: 2.0,
            velocity_threshold: 5.0,
        });
    }
}
```

---

## Implementation Plan

### Phase 1: Core Data Model ✅ COMPLETE

**Goal:** Создать базовую инфраструктуру

- [x] Создать `item_system.rs` модуль
- [x] Типы: `ItemDefinition`, `ItemId`, `ItemType`, `ItemInstance`
- [x] Hardcoded definitions для базовых items:
  - Weapons: `melee_sword`, `pistol_basic`, `rifle_basic`, `dagger`
  - Armor: `armor_military`, `armor_tactical`, `armor_light`, `armor_scrap`
  - Consumables: `health_kit`, `stamina_boost`, `grenade_frag`
- [x] Resource: `ItemDefinitions` (HashMap lookup)
- [x] WeaponStatsTemplate composition refactor (removed field duplication)

**Files:**
- ✅ `crates/voidrun_simulation/src/item_system.rs` (561 lines)
- ✅ `crates/voidrun_simulation/src/lib.rs` (re-exports)

**Tests:** All 4 tests passed

---

### Phase 2: Equipment Components ✅ COMPLETE

**Goal:** Создать новые компоненты, удалить старый Inventory

- [x] Компоненты:
  - `EquippedWeapons` (4 slots + active_slot)
  - `ConsumableSlots` (5 slots + unlock logic)
  - `Armor` component
  - `Inventory` (general storage)
  - `EnergyShield` (with recharge timer fix)
- [x] **УДАЛИТЬ** старый `Inventory` из `inventory.rs`
- [x] **УДАЛИТЬ** `ItemStack` enum (заменён на `ItemInstance`)
- [x] **УДАЛИТЬ** `ActiveWeaponSlot` (заменён на `EquippedWeapons.active_slot`)

**Files:**
- ✅ `crates/voidrun_simulation/src/components/equipment.rs` (510 lines)
- ✅ Deleted `crates/voidrun_simulation/src/components/inventory.rs`
- ✅ `crates/voidrun_simulation/src/components/mod.rs`

**Breaking changes fixed:**
- ✅ Player spawn updated (EquippedWeapons instead of Inventory)
- ✅ Weapon switch migrated to SwapActiveWeaponIntent

**Tests:** All 6 tests passed

---

### Phase 3: Equip/Unequip Systems ✅ COMPLETE

**Goal:** Lifecycle management

- [x] Events:
  - `EquipWeaponIntent`
  - `UnequipWeaponIntent`
  - `SwapActiveWeaponIntent`
  - `EquipArmorIntent`
  - `UnequipArmorIntent`
  - `UseConsumableIntent`
- [x] Systems:
  - `process_equip_weapon` (ItemInstance → WeaponStats + Attachment)
  - `process_unequip_weapon` (remove components, return to Inventory)
  - `process_weapon_swap` (smooth swap активного оружия)
  - `process_equip_armor` (Armor + Attachment + unlock consumables)
  - `process_unequip_armor` (reverse armor effects)
  - `process_use_consumable` (health/stamina/grenade)
- [x] Fix detach: auto-detach old weapon before attaching new
- [x] Update `attach_prefabs_main_thread` (detach logic for empty prefab_path)
- [x] **Runtime Fixes:**
  - Fixed weapon detach not happening on swap
  - Fixed melee intent generated for ranged weapons

**Files:**
- ✅ `crates/voidrun_simulation/src/equipment/mod.rs`
- ✅ `crates/voidrun_simulation/src/equipment/events.rs`
- ✅ `crates/voidrun_simulation/src/equipment/systems.rs` (335 lines)
- ✅ `crates/voidrun_godot/src/systems/attachment_system.rs` (auto-detach logic)
- ✅ `crates/voidrun_godot/src/input/systems.rs` (weapon type routing)

**Additional Work Completed:**
- ✅ Input system migration to Input Map actions (slot1-9, slot0, input_sprint)
- ✅ Architecture validation by architecture-validator agent

---

### Phase 4-7: ⏸️ POSTPONED (Player FPS Shooting Priority)

**Reason:** Critical player experience features take priority over inventory polish.

**Decision Date:** 2025-01-19

**Current Focus:** 🎯 **Player FPS Shooting System** (see new priority section below)

---

#### Phase 4: Weapon Swap Smooth Animations (POSTPONED)

**Original Goal:** Smooth holster → draw transitions

**Status:** ✅ Core swap functionality works (instant switch), smooth animations deferred

**Remaining Work:**
- [ ] HolsterAnimation state
- [ ] DrawAnimation state
- [ ] Animation keyframes trigger detach/attach
- [ ] Smooth transition timing integration

**Files affected:**
- `crates/voidrun_godot/src/systems/weapon_switch.rs` (basic swap working)
- Future: Animation state machine integration

---

#### Phase 5: Consumables Hotbar (PARTIALLY COMPLETE)

**Original Goal:** Hotbar consumables usage (hotkeys 5-9)

**Status:** 🟡 Core system implemented, input routing pending

**Completed:**
- [x] Event: `UseConsumableIntent` ✅
- [x] System: `process_use_consumable` ✅
- [x] Consumable effects (health, stamina) ✅
- [x] Armor unlock logic (2 + bonus slots) ✅
- [x] Input Map actions (`slot5`-`slot9`, `slot0`) ✅

**Remaining Work:**
- [ ] Wire input controller hotkeys 5-9 → `UseConsumableIntent`
- [ ] Grenade/projectile consumables (future)

**Files:**
- ✅ `crates/voidrun_simulation/src/equipment/systems.rs` (process_use_consumable)
- ⏳ `crates/voidrun_godot/src/input/controller.rs` (routing pending)

---

#### Phase 6: AI Integration (POSTPONED)

**Goal:** AI используют equipment систему

**Remaining Work:**
- [ ] AI weapon switching (tactical decisions: distance-based, ammo-based)
- [ ] AI consumable usage (health kit if HP < 30%, stamina boost)
- [ ] AI equipment configuration (faction-based loadouts, shield models)
- [ ] Testing (AI weapon switch, AI consumable usage)

**Files:**
- `crates/voidrun_simulation/src/ai/equipment_ai.rs` (NEW, TBD)
- `crates/voidrun_godot/src/simulation_bridge/spawn.rs` (AI spawn update)

---

#### Phase 7: Polish & Testing (POSTPONED)

**Goal:** Багфиксы, баланс, comprehensive testing

**Remaining Work:**
- [ ] Player spawn с полным equipment (armor, shield, multiple weapons/consumables)
- [ ] AI spawn с разным equipment по faction
- [ ] Comprehensive testing (swap, detach, consumables, AI behavior)
- [ ] Balance tuning
- [ ] Bug fixes

**Files:**
- `crates/voidrun_godot/src/simulation_bridge/mod.rs` (player spawn update)
- `crates/voidrun_godot/src/simulation_bridge/spawn.rs` (NPC spawn update)

---

### 🎯 NEW PRIORITY: Player FPS Shooting System

**Decision:** Postpone equipment polish to focus on critical FPS gameplay mechanics

**Planning Session:** 2025-01-19 (extensive research + architecture design)

**Key Requirements:**
- ✅ Procedural sight alignment (Sight Socket Method from Unreal Engine)
- ✅ Two aim modes: Hip Fire (dynamic raycast) + ADS (sight on camera ray)
- ✅ Manual lerp transitions (NOT keyframe animations - full procedural control)
- ✅ Bullets spawn from barrel (BulletSpawn node), NOT camera center
- ✅ RMB = Toggle ADS для ВСЕХ оружий (including melee for precise strikes)
- ✅ CameraLine debug visualization (hardcoded visible=false)

**Architecture Decisions:**
1. **Sight Socket Method:** Each weapon prefab has `SightSocket` node (artist-configurable)
2. **Manual Lerp:** Smooth transitions via procedural interpolation (0.3s ease-out-cubic)
3. **Continuous Update:** In ADS mode, hand position updates EVERY frame (camera can move!)
4. **No AnimationTree:** Avoid keyframe animation conflicts, full procedural control

**Implementation Phases:**
1. ⏳ Phase 1: Core Systems (AimMode component, helper functions)
2. ⏳ Phase 2: RMB Toggle + Transition Logic (smooth lerp Hip↔ADS)
3. ⏳ Phase 3: Bullet Spawn Fix (from barrel to raycast hit point)
4. ⏳ Phase 4: System Registration (correct Update schedule order)
5. ⏳ Phase 5: Add SightSocket to weapon prefabs

**Documentation:**
- Detailed architecture plan in conversation history (2025-01-19)
- Implementation: `voidrun_godot/src/systems/player_shooting.rs` (TBD)
- Component: `voidrun_simulation/src/components/player_shooting.rs` (TBD)

**Estimated Time:** 6-8 hours (1-2 sessions)

**Resume Equipment Phases 4-7 after:** FPS Shooting System complete and tested

---

### Phase 8: Future (separate sessions)

**Inventory UI:**
- [ ] Godot Control grid
- [ ] Drag-drop между slots
- [ ] Equipable items visualization

**Advanced Features:**
- [ ] ItemDefinitions из RON files (data-driven)
- [ ] Weight/volume limits
- [ ] Dropped weapons как entities (Kenshi-style)
- [ ] Weapon swap animations (holster/draw keyframes)
- [ ] Shield визуал (damage flash effect)

---

## Testing Checklist

### Player Equipment
- [ ] Weapon switch 1-4 работает smooth
- [ ] Detach старого weapon при swap
- [ ] Consumable use 5-9 работает
- [ ] Armor unlocks consumable slots 7-9
- [ ] Shield блокирует ranged урон
- [ ] Melee урон игнорирует shield

### AI Equipment
- [ ] AI переключают оружие (melee ↔ ranged)
- [ ] AI используют health kits (HP < 30%)
- [ ] AI имеют разную броню по faction
- [ ] AI имеют разные shields по faction
- [ ] Military AI имеют лучший equipment

### Edge Cases
- [ ] Weapon swap когда слот пустой (warning)
- [ ] Consumable use когда слот заблокирован (warning)
- [ ] Consumable use когда слот пустой (warning)
- [ ] Durability tracking работает
- [ ] Ammo tracking работает

---

## Known Issues

### Issue 1: Animation Timing
**Problem:** Holster/draw animations не интегрированы
**Impact:** Weapon swap instant (нет smooth transition)
**Fix:** Phase 4 - добавить animation state machine

### Issue 2: Dropped Weapons
**Problem:** Нет системы для drop/pickup weapons
**Impact:** Нельзя собрать loot с трупов
**Fix:** Phase 8 (future) - world items как entities

### Issue 3: RON Definitions
**Problem:** ItemDefinitions hardcoded в коде
**Impact:** Сложно добавлять новые items
**Fix:** Phase 8 (future) - data-driven RON files

---

## References

**Related Docs:**
- [docs/design/shield-technology.md](../design/shield-technology.md) - Shield mechanics
- [docs/architecture/bevy-ecs-design.md](../architecture/bevy-ecs-design.md) - ECS patterns
- [docs/decisions/ADR-007-tscn-prefabs-dynamic-attachment.md](../decisions/ADR-007-tscn-prefabs-dynamic-attachment.md) - Attachment system

**ADRs:**
- ADR-008: Shield Technology Design (shield-technology.md)
- ADR-007: TSCN Prefabs + Dynamic Attachment

---

**Последнее обновление:** 2025-10-19
**Автор:** VOIDRUN Development Team
