# ADR-007: TSCN Prefabs + Rust Dynamic Attachment Pattern

**Дата:** 2025-01-10
**Статус:** ✅ ПРИНЯТО
**Связанные ADR:** [ADR-002](ADR-002-godot-rust-integration-pattern.md), [ADR-004](ADR-004-command-event-architecture.md)

## Контекст

**Проблема:** Как создавать визуальные префабы (актёры, оружие, модули кораблей, предметы) и динамически собирать их в runtime?

**Типичные кейсы:**
- 🎯 **Actor + Weapon:** персонаж держит пистолет/винтовку
- 🚀 **Ship + Modules:** корабль с пушками/щитами/двигателями
- 🚗 **Vehicle + Attachments:** машина с обвесом (бамперы, спойлеры)
- 📦 **Actor + Items:** NPC держит ящик/инструмент
- 🏠 **Building + Furniture:** здание с мебелью/оборудованием

**Требования:**
1. **Godot редактор** — удобно создавать visual префабы (mesh, materials, hierarchy)
2. **Rust логика** — динамически attachить префабы друг к другу (без GDScript export properties)
3. **ECS authoritative** — все решения (что, куда, когда attachить) принимает ECS
4. **Детерминизм** — Save/Load корректно восстанавливает attachments
5. **Type-safety** — компилятор проверяет корректность (нет runtime null reference)

**Почему не GDScript export:**
```gdscript
# ❌ ПРОБЛЕМА (GDScript approach):
export var weapon: PackedScene  # Mutable state в Godot
export var attachment_point: NodePath  # Забыл установить → crash

func _ready():
    var instance = weapon.instantiate()  # Может быть null!
    get_node(attachment_point).add_child(instance)
```

**Проблемы:**
- ❌ Mutable state в Godot (не ECS authoritative)
- ❌ Runtime null reference crashes
- ❌ Save/Load должен сохранять Godot scene state
- ❌ Нет типобезопасности

## Решение

**TSCN Prefabs + Rust Dynamic Attachment Pattern:**

### Архитектура

```
┌──────────────────────────────────────────────────────┐
│ Godot Editor (Asset Creation)                       │
│                                                      │
│ - Visual prefabs (TSCN files)                       │
│ - Marker nodes для attachment points                │
│ - NO scripts, только чистый визуал                  │
└──────────────────────────────────────────────────────┘
                    ↓ (res://... paths)
┌──────────────────────────────────────────────────────┐
│ ECS (Authoritative Logic)                            │
│                                                      │
│ - Components описывают ЧТО attachить                │
│ - Systems принимают решения КОГДА                   │
│ - Source of truth для Save/Load                     │
└──────────────────────────────────────────────────────┘
                    ↓ (Events: Added<T>, Changed<T>)
┌──────────────────────────────────────────────────────┐
│ Godot Systems (Visualization)                        │
│                                                      │
│ - Load TSCN → Instantiate → Attach                  │
│ - Слушают Bevy Change Detection                     │
│ - Рендерят результат (не принимают решения)         │
└──────────────────────────────────────────────────────┘
```

**Ключевое:** ECS = what/when, Godot = how (rendering).

### Универсальная структура TSCN Prefab

#### Host Prefab (то, К ЧЕМУ attachим)

```gdscene
# === test_actor.tscn ===

[node name="Actor" type="Node3D"]

[node name="Head" type="MeshInstance3D" parent="."]
mesh = SubResource("...")

[node name="Torso" type="MeshInstance3D" parent="."]
mesh = SubResource("...")

[node name="RightHand" type="MeshInstance3D" parent="."]
mesh = SubResource("...")

# ВАЖНО: Attachment Point — пустая Node3D с именованным путём
[node name="WeaponAttachment" type="Node3D" parent="RightHand"]
# ^^^ Сюда будет attachиться weapon prefab (primary point)

# ВАЖНО: IK Target для двуручных предметов
[node name="RightHandIK" type="Node3D" parent="RightHand"]
# ^^^ Target для IK constraint (если нужно)

[node name="LeftHand" type="MeshInstance3D" parent="."]
mesh = SubResource("...")

[node name="ItemAttachment" type="Node3D" parent="LeftHand"]
# ^^^ Сюда может attachиться item (ящик, инструмент)

# ВАЖНО: IK Target для левой руки (для двуручных предметов)
[node name="LeftHandIK" type="Node3D" parent="LeftHand"]
# ^^^ Secondary attachment point для two-handed items
```

**Naming Convention для Attachment Points:**
- `{Purpose}Attachment` — например `WeaponAttachment`, `ShieldAttachment`, `EngineAttachment` (primary points)
- `{Purpose}IK` — например `LeftHandIK`, `RightHandIK` (secondary points для IK constraints)
- Всегда `type="Node3D"` (пустой pivot для attachment)
- Позиция/rotation в transform — определяет положение attached prefab'а

#### Attachable Prefab (то, ЧТО attachим)

**Single-Point (одноручное оружие):**

```gdscene
# === test_pistol.tscn ===

[node name="Pistol" type="Node3D"]

# ВАЖНО: Root pivot — определяет ориентацию в attachment point
[node name="WeaponPlacement" type="Node3D" parent="."]
transform = Transform3D(...)  # Offset/rotation для правильного положения

[node name="Mesh" type="MeshInstance3D" parent="WeaponPlacement"]
mesh = SubResource("...")

# ВАЖНО: Marker nodes для gameplay logic
[node name="BulletSpawn" type="Node3D" parent="WeaponPlacement"]
# ^^^ Откуда вылетает пуля (можно query из Rust)

[node name="MuzzleFlash" type="GPUParticles3D" parent="WeaponPlacement"]
# ^^^ Эффекты
```

**Multi-Point (двуручное оружие):**

```gdscene
# === rifle.tscn ===

[node name="Rifle" type="Node3D"]

[node name="WeaponPlacement" type="Node3D" parent="."]
transform = Transform3D(...)

[node name="Mesh" type="MeshInstance3D" parent="WeaponPlacement"]
mesh = SubResource("...")

# ВАЖНО: Grip Points для IK constraints
[node name="GripPoints" type="Node3D" parent="WeaponPlacement"]

# Правая рука держит за рукоять (primary, автоматически через WeaponAttachment)
[node name="RightGrip" type="Node3D" parent="GripPoints"]
transform = Transform3D(...)  # Позиция рукояти

# Левая рука держит за цевьё (secondary, через IK constraint)
[node name="LeftGrip" type="Node3D" parent="GripPoints"]
transform = Transform3D(...)  # Позиция цевья

[node name="BulletSpawn" type="Node3D" parent="WeaponPlacement"]
```

**Naming Convention для Markers:**
- `{Purpose}Spawn` — для spawn points (BulletSpawn, MissileSpawn)
- `{Feature}Point` — для gameplay маркеров (GripPoint, AimPoint)
- `GripPoints/{Side}Grip` — для multi-point attachments (LeftGrip, RightGrip)
- Godot системы могут query эти nodes по имени

### ECS Component Pattern

**Универсальный компонент для attachment (поддерживает single-point и multi-point):**

```rust
// === voidrun_simulation/src/components/attachment.rs ===

/// Универсальный компонент для dynamic attachment (одноручные и двуручные предметы)
#[derive(Component, Clone, Debug)]
pub struct Attachment {
    /// Prefab который attachим (res://...)
    pub prefab_path: String,

    /// Primary attachment point внутри host prefab'а (node path)
    pub attachment_point: String,

    /// Тип attachment (для validation/filtering)
    pub attachment_type: AttachmentType,

    /// Secondary attachment points (для двуручных предметов, IK constraints)
    /// Если пустой → single-point attachment (одноручное)
    pub secondary_points: Vec<SecondaryAttachmentPoint>,
}

/// Secondary attachment point для multi-point attachments (двуручные предметы)
#[derive(Clone, Debug)]
pub struct SecondaryAttachmentPoint {
    /// Node path в host prefab'е (например "LeftHand/LeftHandIK")
    pub host_point: String,

    /// Node path внутри attachable prefab'а (например "GripPoints/LeftGrip")
    pub prefab_marker: String,

    /// Тип constraint (IK, Position, LookAt)
    pub constraint_type: ConstraintType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintType {
    /// IK constraint (рука/нога тянется к маркеру)
    IK,

    /// Position constraint (node следует за позицией маркера)
    Position,

    /// LookAt constraint (node смотрит на маркер)
    LookAt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentType {
    Weapon,
    ShipModule,
    VehicleAccessory,
    Item,
    Equipment,
}

impl Attachment {
    /// Helper: создать одноручное weapon attachment
    pub fn weapon(weapon_prefab: &str) -> Self {
        Self {
            prefab_path: weapon_prefab.into(),
            attachment_point: "RightHand/WeaponAttachment".into(),
            attachment_type: AttachmentType::Weapon,
            secondary_points: vec![], // Single-point (одноручное)
        }
    }

    /// Helper: создать двуручную винтовку
    pub fn two_handed_rifle(rifle_prefab: &str) -> Self {
        Self {
            prefab_path: rifle_prefab.into(),
            attachment_point: "RightHand/WeaponAttachment".into(),
            attachment_type: AttachmentType::Weapon,
            secondary_points: vec![
                SecondaryAttachmentPoint {
                    host_point: "LeftHand/LeftHandIK".into(),
                    prefab_marker: "GripPoints/LeftGrip".into(),
                    constraint_type: ConstraintType::IK,
                },
            ],
        }
    }

    /// Helper: создать двуручный меч
    pub fn two_handed_sword(sword_prefab: &str) -> Self {
        Self {
            prefab_path: sword_prefab.into(),
            attachment_point: "RightHand/WeaponAttachment".into(),
            attachment_type: AttachmentType::Weapon,
            secondary_points: vec![
                SecondaryAttachmentPoint {
                    host_point: "LeftHand/LeftHandIK".into(),
                    prefab_marker: "GripPoints/LeftGrip".into(),
                    constraint_type: ConstraintType::IK,
                },
            ],
        }
    }

    /// Helper: создать тяжёлую коробку (обе руки)
    pub fn heavy_crate(crate_prefab: &str) -> Self {
        Self {
            prefab_path: crate_prefab.into(),
            attachment_point: "RightHand/ItemAttachment".into(),
            attachment_type: AttachmentType::Item,
            secondary_points: vec![
                SecondaryAttachmentPoint {
                    host_point: "LeftHand/LeftHandIK".into(),
                    prefab_marker: "GripPoints/LeftHandle".into(),
                    constraint_type: ConstraintType::IK,
                },
            ],
        }
    }

    /// Helper: создать ship module attachment
    pub fn ship_module(module_prefab: &str, slot: &str) -> Self {
        Self {
            prefab_path: module_prefab.into(),
            attachment_point: format!("Hardpoints/{}", slot),
            attachment_type: AttachmentType::ShipModule,
            secondary_points: vec![],
        }
    }

    /// Helper: создать лёгкий item attachment (одноручный)
    pub fn item(item_prefab: &str) -> Self {
        Self {
            prefab_path: item_prefab.into(),
            attachment_point: "LeftHand/ItemAttachment".into(),
            attachment_type: AttachmentType::Item,
            secondary_points: vec![],
        }
    }

    /// Проверка: is two-handed attachment?
    pub fn is_two_handed(&self) -> bool {
        !self.secondary_points.is_empty()
    }
}
```

**Множественные attachments:**

```rust
/// Компонент для хранения нескольких attachments (например модули корабля)
#[derive(Component, Clone, Debug, Default)]
pub struct AttachmentSlots {
    pub slots: HashMap<String, Attachment>,
}

impl AttachmentSlots {
    /// Добавить attachment в слот
    pub fn insert(&mut self, slot_name: String, attachment: Attachment) {
        self.slots.insert(slot_name, attachment);
    }

    /// Убрать attachment из слота
    pub fn remove(&mut self, slot_name: &str) -> Option<Attachment> {
        self.slots.remove(slot_name)
    }
}
```

### Godot System Pattern

**Универсальная система для attachment:**

```rust
// === voidrun_godot/src/systems/attachment_system.rs ===

use godot::prelude::*;
use bevy::prelude::*;
use voidrun_simulation::Attachment;

/// Resource для хранения attached nodes
#[derive(Resource, Default)]
pub struct AttachmentRegistry {
    /// HashMap<(Entity, AttachmentPoint), Gd<Node3D>>
    /// Key = (entity ID, attachment point path)
    pub attachments: HashMap<(Entity, String), Gd<Node3D>>,
}

/// Система: attach prefabs для Added<Attachment>
pub fn attach_prefabs(
    query: Query<(Entity, &Attachment), Added<Attachment>>,
    visuals: Res<VisualRegistry>, // HashMap<Entity, Gd<Node3D>> host prefabs
    mut registry: ResMut<AttachmentRegistry>,
) {
    for (entity, attachment) in query.iter() {
        // 1. Получить host node (actor/ship/vehicle)
        let host_node = match visuals.visuals.get(&entity) {
            Some(node) => node,
            None => {
                godot_warn!(
                    "Entity {:?} has Attachment but no visual prefab yet",
                    entity
                );
                continue;
            }
        };

        // 2. Найти attachment point внутри host prefab'а
        let attachment_point = match host_node.try_get_node_as::<Node3D>(&attachment.attachment_point) {
            Some(node) => node,
            None => {
                godot_error!(
                    "Host prefab for {:?} missing attachment point: {}",
                    entity,
                    attachment.attachment_point
                );
                continue;
            }
        };

        // 3. Load attachable prefab
        let prefab_scene = load::<PackedScene>(&attachment.prefab_path);
        let mut prefab_instance = prefab_scene.instantiate_as::<Node3D>();

        // 4. Attach meta для обратного маппинга
        prefab_instance.set_meta("owner_entity".into(), entity.index().to_variant());
        prefab_instance.set_meta("attachment_type".into(),
            format!("{:?}", attachment.attachment_type).to_variant()
        );

        // 5. Attach к host (primary point)
        attachment_point.add_child(prefab_instance.clone());

        // 6. Setup secondary attachment points (для двуручных предметов)
        for secondary in &attachment.secondary_points {
            setup_secondary_constraint(&host_node, &prefab_instance, secondary);
        }

        // 7. Сохранить в registry
        let key = (entity, attachment.attachment_point.clone());
        registry.attachments.insert(key, prefab_instance);

        godot_print!(
            "Attached {:?} to entity {:?} at {} (secondary points: {})",
            attachment.prefab_path,
            entity,
            attachment.attachment_point,
            attachment.secondary_points.len()
        );
    }
}

/// Система: detach при RemovedComponents<Attachment>
pub fn detach_prefabs(
    mut removed: RemovedComponents<Attachment>,
    mut registry: ResMut<AttachmentRegistry>,
) {
    for entity in removed.read() {
        // Удалить все attachments для этого entity
        registry.attachments.retain(|(ent, _), node| {
            if *ent == entity {
                node.queue_free();
                godot_print!("Detached attachment from entity {:?}", entity);
                false
            } else {
                true
            }
        });
    }
}

/// Система: reattach при Changed<Attachment> (swap attachments)
pub fn reattach_changed_prefabs(
    query: Query<(Entity, &Attachment), Changed<Attachment>>,
    visuals: Res<VisualRegistry>,
    mut registry: ResMut<AttachmentRegistry>,
) {
    for (entity, attachment) in query.iter() {
        let key = (entity, attachment.attachment_point.clone());

        // 1. Удалить старый attachment
        if let Some(old_node) = registry.attachments.remove(&key) {
            old_node.queue_free();
        }

        // 2. Attach новый (тот же код что в attach_prefabs)
        let host_node = visuals.visuals.get(&entity).unwrap();
        let attachment_point = host_node.get_node_as::<Node3D>(&attachment.attachment_point);

        let prefab_scene = load::<PackedScene>(&attachment.prefab_path);
        let prefab_instance = prefab_scene.instantiate_as::<Node3D>();

        attachment_point.add_child(prefab_instance.clone());
        registry.attachments.insert(key, prefab_instance);

        godot_print!("Reattached {:?} for entity {:?}", attachment.prefab_path, entity);
    }
}

/// Helper: setup secondary constraint (IK, Position, LookAt)
fn setup_secondary_constraint(
    host_node: &Gd<Node3D>,
    prefab_instance: &Gd<Node3D>,
    secondary: &SecondaryAttachmentPoint,
) {
    // 1. Получить host IK target node
    let host_ik_target = match host_node.try_get_node_as::<Node3D>(&secondary.host_point) {
        Some(node) => node,
        None => {
            godot_error!("Missing host IK target: {}", secondary.host_point);
            return;
        }
    };

    // 2. Получить marker внутри prefab'а
    let prefab_marker = match prefab_instance.try_get_node_as::<Node3D>(&secondary.prefab_marker) {
        Some(node) => node,
        None => {
            godot_error!("Missing prefab marker: {}", secondary.prefab_marker);
            return;
        }
    };

    // 3. Setup constraint в зависимости от типа
    match secondary.constraint_type {
        ConstraintType::IK => {
            // IK constraint через RemoteTransform3D (простой вариант)
            let mut remote_transform = RemoteTransform3D::new_alloc();
            remote_transform.set_remote_node(prefab_marker.get_path());
            remote_transform.set_update_position(true);
            remote_transform.set_update_rotation(true);
            host_ik_target.add_child(remote_transform.clone());

            godot_print!("Setup IK constraint: {:?} -> {:?}",
                host_ik_target.get_name(),
                prefab_marker.get_name()
            );
        }
        ConstraintType::Position => {
            // Position constraint (только позиция)
            let mut remote_transform = RemoteTransform3D::new_alloc();
            remote_transform.set_remote_node(prefab_marker.get_path());
            remote_transform.set_update_position(true);
            remote_transform.set_update_rotation(false);
            host_ik_target.add_child(remote_transform.clone());
        }
        ConstraintType::LookAt => {
            // LookAt constraint (можно через custom node или look_at() в системе)
            godot_print!("LookAt constraint setup для {:?}", host_ik_target.get_name());
        }
    }
}
```

**Система для множественных attachments:**

```rust
/// Attach/detach для AttachmentSlots (корабль с модулями)
pub fn sync_attachment_slots(
    query: Query<(Entity, &AttachmentSlots), Changed<AttachmentSlots>>,
    visuals: Res<VisualRegistry>,
    mut registry: ResMut<AttachmentRegistry>,
) {
    for (entity, slots) in query.iter() {
        let host_node = visuals.visuals.get(&entity).unwrap();

        // Detach все старые attachments для этого entity
        registry.attachments.retain(|(ent, _), node| {
            if *ent == entity {
                node.queue_free();
                false
            } else {
                true
            }
        });

        // Attach новые из slots
        for (slot_name, attachment) in slots.slots.iter() {
            let attachment_point = host_node.get_node_as::<Node3D>(&attachment.attachment_point);

            let prefab_scene = load::<PackedScene>(&attachment.prefab_path);
            let prefab_instance = prefab_scene.instantiate_as::<Node3D>();

            attachment_point.add_child(prefab_instance.clone());

            let key = (entity, attachment.attachment_point.clone());
            registry.attachments.insert(key, prefab_instance);
        }
    }
}
```

### Animation Stance Integration

**Проблема:** При смене оружия (одноручное → двуручное) нужно переключать AnimationTree state.

**Решение:** Автоматическое определение `WeaponStance` из `Attachment` компонента.

```rust
// === voidrun_simulation/src/animation/components.rs ===

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum WeaponStance {
    Unarmed,
    OneHandedPistol,
    TwoHandedRifle,
    TwoHandedSword,
    HeavyItem,
}

impl From<&Attachment> for WeaponStance {
    fn from(attachment: &Attachment) -> Self {
        // Определяем stance по количеству secondary points
        if attachment.secondary_points.is_empty() {
            // Одноручное
            match attachment.attachment_type {
                AttachmentType::Weapon => WeaponStance::OneHandedPistol,
                AttachmentType::Item => WeaponStance::Unarmed,
                _ => WeaponStance::Unarmed,
            }
        } else {
            // Двуручное (есть secondary points)
            match attachment.attachment_type {
                AttachmentType::Weapon => {
                    // Можно различать по prefab path
                    if attachment.prefab_path.contains("rifle") {
                        WeaponStance::TwoHandedRifle
                    } else {
                        WeaponStance::TwoHandedSword
                    }
                }
                AttachmentType::Item => WeaponStance::HeavyItem,
                _ => WeaponStance::Unarmed,
            }
        }
    }
}
```

**ECS система автоматически устанавливает stance:**

```rust
// === voidrun_simulation/src/animation/systems.rs ===

/// Автоматически устанавливаем WeaponStance при Added/Changed<Attachment>
pub fn update_weapon_stance(
    query: Query<(Entity, &Attachment), Or<(Added<Attachment>, Changed<Attachment>)>>,
    mut commands: Commands,
) {
    for (entity, attachment) in query.iter() {
        let stance = WeaponStance::from(attachment);
        commands.entity(entity).insert(stance);
    }
}

/// Очищаем stance при удалении оружия
pub fn clear_weapon_stance_on_detach(
    mut removed: RemovedComponents<Attachment>,
    mut commands: Commands,
) {
    for entity in removed.read() {
        commands.entity(entity).insert(WeaponStance::Unarmed);
    }
}
```

**Godot система sync AnimationTree:**

```rust
// === voidrun_godot/src/systems/animation_stance_sync.rs ===

pub fn sync_animation_stance(
    query: Query<(Entity, &WeaponStance), Changed<WeaponStance>>,
    visuals: Res<VisualRegistry>,
) {
    for (entity, stance) in query.iter() {
        let actor_node = match visuals.visuals.get(&entity) {
            Some(node) => node,
            None => continue,
        };

        // Получить AnimationTree
        let mut anim_tree = match actor_node.try_get_node_as::<AnimationTree>("AnimationTree") {
            Some(tree) => tree,
            None => continue,
        };

        // Переключить state в AnimationTree
        let state_machine_path = "parameters/StanceStateMachine/transition_request";
        let next_state = match stance {
            WeaponStance::Unarmed => "unarmed_idle",
            WeaponStance::OneHandedPistol => "pistol_idle",
            WeaponStance::TwoHandedRifle => "rifle_idle",
            WeaponStance::TwoHandedSword => "sword_idle",
            WeaponStance::HeavyItem => "carry_idle",
        };

        anim_tree.set(state_machine_path.into(), next_state.to_variant());

        godot_print!(
            "Switched animation stance for entity {:?} to {:?}",
            entity,
            next_state
        );
    }
}
```

**Workflow:**
1. ECS: `commands.entity(player).insert(Attachment::two_handed_rifle(...))`
2. ECS система: `update_weapon_stance` → `WeaponStance::TwoHandedRifle`
3. Godot система: `attach_prefabs` → attach rifle + setup IK
4. Godot система: `sync_animation_stance` → AnimationTree "rifle_idle"
5. Результат: Actor держит винтовку двумя руками + правильная анимация ✅

## Примеры использования

### Пример 1: Actor + Weapon

**TSCN Setup:**
```gdscene
# actor.tscn
[node name="RightHand" type="MeshInstance3D"]
[node name="WeaponAttachment" type="Node3D" parent="RightHand"]

# pistol.tscn
[node name="Pistol" type="Node3D"]
[node name="WeaponPlacement" type="Node3D" parent="."]
[node name="BulletSpawn" type="Node3D" parent="WeaponPlacement"]
```

**ECS Spawn:**
```rust
commands.spawn((
    VisualPrefab { path: "res://actors/test_actor.tscn".into() },
    Attachment::weapon("res://weapons/test_pistol.tscn"),
    Health { current: 100.0, max: 100.0 },
));
```

**Godot Timeline:**
1. Frame 1: ECS spawn
2. Frame 2: `spawn_visual_for_entities` → instantiate actor
3. Frame 3: `attach_prefabs` → load pistol → attach к RightHand/WeaponAttachment
4. Frame 4: Actor держит пистолет ✅

### Пример 2: Ship + Modules

**TSCN Setup:**
```gdscene
# ship.tscn
[node name="Hardpoints" type="Node3D"]
[node name="Weapon1" type="Node3D" parent="Hardpoints"]
[node name="Weapon2" type="Node3D" parent="Hardpoints"]
[node name="Shield" type="Node3D" parent="Hardpoints"]

# laser_cannon.tscn
[node name="LaserCannon" type="Node3D"]
[node name="MuzzlePoint" type="Node3D" parent="."]
```

**ECS Spawn:**
```rust
let mut slots = AttachmentSlots::default();
slots.insert(
    "Weapon1".into(),
    Attachment::ship_module("res://modules/laser_cannon.tscn", "Weapon1")
);
slots.insert(
    "Weapon2".into(),
    Attachment::ship_module("res://modules/missile_launcher.tscn", "Weapon2")
);
slots.insert(
    "Shield".into(),
    Attachment::ship_module("res://modules/energy_shield.tscn", "Shield")
);

commands.spawn((
    VisualPrefab { path: "res://ships/fighter.tscn".into() },
    slots,
    ShipStats { hull: 500.0, shields: 200.0 },
));
```

### Пример 3: Vehicle + Accessories

**TSCN Setup:**
```gdscene
# car.tscn
[node name="FrontBumper" type="Node3D"]
[node name="RearBumper" type="Node3D"]
[node name="Spoiler" type="Node3D"]
```

**ECS Spawn:**
```rust
let mut slots = AttachmentSlots::default();
slots.insert("FrontBumper".into(), Attachment {
    prefab_path: "res://accessories/bumper_racing.tscn".into(),
    attachment_point: "FrontBumper".into(),
    attachment_type: AttachmentType::VehicleAccessory,
});

commands.spawn((
    VisualPrefab { path: "res://vehicles/car.tscn".into() },
    slots,
));
```

### Пример 4: Actor + Item (симуляция жизни)

**ECS Spawn:**
```rust
commands.spawn((
    VisualPrefab { path: "res://actors/trader.tscn".into() },
    Attachment::item("res://items/crate.tscn"), // Trader держит ящик
    AIState::CarryingCargo,
));
```

## Работа с Marker Nodes

**Получить позицию BulletSpawn при стрельбе:**

```rust
// === voidrun_godot/src/combat/shooting.rs ===

pub fn get_bullet_spawn_position(
    entity: Entity,
    registry: &AttachmentRegistry,
) -> Option<Vec3> {
    // Получить weapon node
    let key = (entity, "RightHand/WeaponAttachment".to_string());
    let weapon_node = registry.attachments.get(&key)?;

    // Найти BulletSpawn marker внутри weapon prefab'а
    let bullet_spawn = weapon_node.try_get_node_as::<Node3D>("WeaponPlacement/BulletSpawn")?;

    // Global position (учитывает все parent transforms)
    let pos = bullet_spawn.get_global_position();
    Some(Vec3::new(pos.x, pos.y, pos.z))
}
```

**Использование в ECS:**

```rust
pub fn handle_shoot_event(
    mut events: EventReader<ShootEvent>,
    registry: Res<AttachmentRegistry>,
    mut commands: Commands,
) {
    for event in events.read() {
        if let Some(spawn_pos) = get_bullet_spawn_position(event.shooter, &registry) {
            commands.spawn((
                Bullet { velocity: event.direction * 50.0 },
                Transform::from_translation(spawn_pos),
            ));
        }
    }
}
```

## Save/Load

**Компонент сохраняется в ECS:**

```rust
#[derive(Serialize, Deserialize)]
struct SavedEntity {
    visual_prefab: String,
    attachment: Option<Attachment>, // Или AttachmentSlots
    health: f32,
    // ...
}
```

**При Load:**
1. ECS восстанавливает entity с `Attachment` component
2. Godot системы видят `Added<Attachment>` → автоматически attach prefab
3. Визуал полностью восстановлен ✅

**Детерминизм:**
- ECS = source of truth (что attached)
- Godot = презентация (как выглядит)
- Save/Load не зависит от Godot scene state

## Обоснование

### Почему TSCN Prefabs

**Преимущества:**
- ✅ **Godot Editor UX** — удобно создавать визуальные префабы (mesh, materials, hierarchy)
- ✅ **Artist-friendly** — художники работают в редакторе, не трогают код
- ✅ **Hot-reload** — изменения в TSCN видны сразу (не нужна перекомпиляция Rust)
- ✅ **Asset reuse** — один prefab используется многократно

**Trade-offs:**
- ⚠️ **Path strings** — `"res://..."` пути (не типобезопасны как Rust энумы)
- ⚠️ **Runtime errors** — если prefab не найден или attachment point отсутствует

**Митигация:**
- Asset validation система (проверяет что все пути корректны при startup)
- Graceful fallback (`try_get_node_as()` вместо `.unwrap()`)

### Почему Rust Dynamic Attachment

**Преимущества:**
- ✅ **ECS authoritative** — все решения в Rust (типобезопасность)
- ✅ **Детерминизм** — Save/Load не зависит от Godot state
- ✅ **Hot-swap** — можно менять attachments runtime (смена оружия, модулей)
- ✅ **Модульность** — один паттерн для всех типов attachments

**Trade-offs:**
- ⚠️ **Boilerplate** — нужны Godot системы для каждого типа attachment
- ⚠️ **Frame delay** — attachment появляется через 1-2 frame после ECS spawn

**Почему НЕ GDScript export:**
- ❌ Mutable state в Godot (не детерминистично)
- ❌ Runtime null crashes
- ❌ Save/Load сложнее (нужно сохранять Godot scene state)

## Влияния

### Новые компоненты

**voidrun_simulation/src/components/attachment.rs:**
```rust
pub struct Attachment {
    prefab_path,
    attachment_point,
    attachment_type,
    secondary_points: Vec<SecondaryAttachmentPoint>, // NEW: для двуручных предметов
}

pub struct SecondaryAttachmentPoint {
    host_point,        // "LeftHand/LeftHandIK"
    prefab_marker,     // "GripPoints/LeftGrip"
    constraint_type,   // IK, Position, LookAt
}

pub enum ConstraintType { IK, Position, LookAt }

pub struct AttachmentSlots { slots: HashMap<String, Attachment> }
pub enum AttachmentType { Weapon, ShipModule, VehicleAccessory, Item, Equipment }
```

**voidrun_simulation/src/animation/components.rs:**
```rust
pub enum WeaponStance {
    Unarmed,
    OneHandedPistol,
    TwoHandedRifle,
    TwoHandedSword,
    HeavyItem,
}
```

### Новые системы

**voidrun_simulation/src/animation/systems.rs:**
- `update_weapon_stance` — автоопределение stance из Attachment
- `clear_weapon_stance_on_detach` — очистка при удалении оружия

**voidrun_godot/src/systems/attachment_system.rs:**
- `attach_prefabs` — Added<Attachment> (с поддержкой secondary points)
- `detach_prefabs` — RemovedComponents<Attachment>
- `reattach_changed_prefabs` — Changed<Attachment>
- `sync_attachment_slots` — Changed<AttachmentSlots>
- `setup_secondary_constraint()` — helper для IK/Position/LookAt constraints

**voidrun_godot/src/systems/animation_stance_sync.rs:**
- `sync_animation_stance` — Changed<WeaponStance> → AnimationTree sync

### Новые ресурсы

**voidrun_godot:**
```rust
pub struct AttachmentRegistry {
    attachments: HashMap<(Entity, String), Gd<Node3D>>,
}
```

### TSCN Naming Conventions

**Host Prefabs (actors, ships, vehicles):**
- Primary attachment points: `{Purpose}Attachment` (WeaponAttachment, ShieldAttachment)
- Secondary attachment points (IK targets): `{Purpose}IK` (LeftHandIK, RightHandIK)
- Всегда `type="Node3D"` пустые pivots

**Attachable Prefabs (weapons, modules, items):**
- Root: `{PrefabName}` (Pistol, LaserCannon, Crate)
- Placement pivot: `{Type}Placement` (WeaponPlacement, ModulePlacement)
- Markers: `{Purpose}Spawn` (BulletSpawn, MissileSpawn)
- Grip points (для двуручных): `GripPoints/{Side}Grip` (LeftGrip, RightGrip)

### App Setup

**voidrun_simulation:**
```rust
app.add_systems(Update, (
    // ECS animation stance
    update_weapon_stance,
    clear_weapon_stance_on_detach,
));
```

**voidrun_godot:**
```rust
app.add_systems(Update, (
    // Attachment systems
    attach_prefabs,
    detach_prefabs,
    reattach_changed_prefabs,
    sync_attachment_slots,

    // Animation sync
    sync_animation_stance,
));
```

## Риски и митигация

### Риск 1: Неправильный attachment_point path

**Описание:** Typo в пути → attachment не работает.

**Вероятность:** Средняя (строковые пути)

**Влияние:** Среднее (attachment невидим, но игра не крашится)

**Митигация:**
- Asset validation система (проверяет пути при startup)
- Const helpers: `const WEAPON_ATTACHMENT: &str = "RightHand/WeaponAttachment"`
- Graceful fallback (godot_error! + continue, не panic)

**Метрики:**
- Asset validation errors = 0 (OK)
- Runtime attachment failures > 5% (проблема)

### Риск 2: Prefab не найден (incorrect res:// path)

**Описание:** Typo в `prefab_path` → load fails.

**Вероятность:** Средняя

**Влияние:** Среднее (attachment не появляется)

**Митигация:**
- Centralized prefab registry (enum → path mapping)
- Asset validation при startup
- Fallback на default prefab (например placeholder cube)

### Риск 3: Memory leaks (забыли queue_free)

**Описание:** Detach не вызывает `queue_free()` → nodes накапливаются.

**Вероятность:** Низкая (код явный)

**Влияние:** Высокое (memory leak)

**Митигация:**
- RAII pattern — `AttachmentRegistry` owns nodes
- Drop impl для cleanup
- Memory profiling в CI

### Риск 4: Frame delay (attachment появляется позже spawn)

**Описание:** ECS spawn → Frame 1, attachment → Frame 2-3.

**Вероятность:** 100% (by design)

**Влияние:** Низкое (визуальный delay, не gameplay bug)

**Митигация:**
- Spawn за пределами viewport (player не видит delay)
- Loading screen для first spawn
- Accept delay (1-2 frame = 16-32ms, незаметно)

## Альтернативы (отклонены)

### Вариант A: GDScript export properties

```gdscript
export var weapon: PackedScene
export var attachment_point: NodePath

func _ready():
    var instance = weapon.instantiate()
    get_node(attachment_point).add_child(instance)
```

**Почему отклонено:**
- ❌ Не ECS authoritative (state в Godot)
- ❌ Save/Load сложнее
- ❌ Нет типобезопасности
- ❌ Противоречит Rust-only policy

### Вариант B: Hardcode attachments в TSCN

```gdscene
[node name="RightHand" type="Node3D"]
[node name="Pistol" parent="RightHand" instance=ExtResource("res://pistol.tscn")]
```

**Почему отклонено:**
- ❌ Нет runtime flexibility (нельзя swap оружие)
- ❌ Все комбинации = отдельные TSCN (explosion of variants)
- ❌ Save/Load должен хранить какой именно variant

### Вариант C: Процедурная генерация mesh'ей в Rust

```rust
let mesh = CubeMesh::new();
mesh.set_size(Vector3::new(1.0, 2.0, 0.5));
mesh_instance.set_mesh(mesh);
```

**Почему отклонено:**
- ❌ Сложно создавать сложные модели (потеряем Godot Editor UX)
- ❌ Художники не могут работать (Rust код вместо редактора)
- ❌ Hot-reload медленнее (Rust compile vs TSCN reload)

**Когда использовать:**
- ✅ Простые placeholder'ы (debug cubes, debug spheres)
- ✅ Процедурная генерация (terrain, noise-based geometry)

## План имплементации

### Фаза 1: Core Components (1-2 часа)

1. `voidrun_simulation/src/components/attachment.rs`
   - `Attachment` struct
   - `AttachmentSlots` struct
   - `AttachmentType` enum
   - Helper methods (weapon(), ship_module(), item())

### Фаза 2: Godot Systems (2-3 часа)

2. `voidrun_godot/src/systems/attachment_system.rs`
   - `AttachmentRegistry` resource
   - `attach_prefabs` система
   - `detach_prefabs` система
   - `reattach_changed_prefabs` система

3. App setup — register systems

### Фаза 3: TSCN Prefabs (1-2 часа)

4. Обновить `test_actor.tscn`
   - Добавить `WeaponAttachment` node
   - Добавить `ItemAttachment` node

5. Создать `test_pistol.tscn`
   - `WeaponPlacement` pivot
   - `BulletSpawn` marker
   - Mesh'и

### Фаза 4: Integration (1-2 часа)

6. Обновить spawn системы
   - `spawn_test_actor` с `Attachment::weapon()`
   - Smoke test в Godot

7. Marker query helpers
   - `get_bullet_spawn_position()`
   - `get_grip_point()`

### Фаза 5: Advanced Features (2-3 часа)

8. `AttachmentSlots` поддержка (корабли с модулями)
9. Asset validation система (проверка путей)
10. Centralized prefab registry (enum → path)

**Итого:** 7-12 часов (~1-1.5 дня)

## Откат

Если подход не зайдёт:

**План B: Hybrid (TSCN + hardcoded attachments)**
- Простые кейсы → hardcode в TSCN
- Сложные (runtime swap) → Rust attachment
- Компромисс flexibility vs simplicity

**План C: Full Rust Procedural**
- Всё создаётся процедурно в Rust
- Максимум flexibility, минимум artist UX
- Для прототипа OK, для production проблема

**Критерии для отката:**
- Asset validation слишком сложна (много false positives)
- Frame delay заметен игрокам (>100ms)
- Художники не могут работать (TSCN workflow сломан)

**Вероятность отката:** <5%

## Заключение

**TSCN Prefabs + Rust Dynamic Attachment** = универсальный паттерн для композиции визуальных префабов.

**Ключевые принципы:**
- **Godot = asset storage** — TSCN префабы для визуала
- **ECS = authoritative logic** — компоненты описывают attachments
- **Rust systems = glue** — слушают Change Detection → attach/detach
- **Детерминизм** — Save/Load восстанавливает из ECS state

**Универсальность:**
- 🎯 Actor + Weapon
- 🚀 Ship + Modules
- 🚗 Vehicle + Accessories
- 📦 Actor + Items
- 🏠 Building + Furniture

**Все используют один паттерн:**
```rust
Attachment {
    prefab_path,
    attachment_point,
    attachment_type,
    secondary_points // Для multi-point (two-handed, IK)
}
```

**Multi-point support:**
- 🤺 Two-handed weapons (rifle, sword)
- 📦 Heavy items (crate, barrel)
- 🤖 IK constraints (hand grips)
- 🔗 Complex attachments (любое кол-во точек)

**Следующие шаги:** См. План имплементации (Фаза 1-5).

---

**См. также:**
- [ADR-002: Godot-Rust Integration Pattern](ADR-002-godot-rust-integration-pattern.md) — Rust-centric подход
- [ADR-004: Command/Event Architecture](ADR-004-command-event-architecture.md) — Bevy Events для sync
- [godot-rust-integration.md](../architecture/godot-rust-integration.md) — Rust-only policy
