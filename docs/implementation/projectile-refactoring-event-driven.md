# Projectile System Refactoring (Event-Driven)

**Статус:** 📋 Ready to implement
**Версия:** 1.1
**Дата:** 2025-10-25
**Оценка:** 4.5-5.5 часов (полдня работы)

---

## 📋 Обзор

Рефакторинг projectile системы с `static mut PROJECTILE_HIT_QUEUE` на event-driven архитектуру. Проблема текущей реализации: использование unsafe global state для передачи событий из Godot в ECS. Решение: Bevy events для clean separation.

---

## ❌ Текущая проблема

**Файл:** `crates/voidrun_godot/src/projectile.rs`

**Проблемные паттерны:**
```rust
// ❌ UNSAFE GLOBAL STATE
static mut PROJECTILE_HIT_QUEUE: Option<Vec<ProjectileHit>> = None;
static mut NODE_TO_ENTITY: Option<HashMap<InstanceId, Entity>> = None;

// ❌ Godot code напрямую пишет в queue
unsafe {
    if let Some(queue) = PROJECTILE_HIT_QUEUE.as_mut() {
        queue.push(ProjectileHit { ... });
    }
}

// ❌ ECS система читает queue
pub fn process_godot_projectile_hits(
    mut projectile_hit_events: EventWriter<ProjectileHit>,
) {
    let hits = take_projectile_hits();  // Забирает из static mut
    for hit in hits {
        projectile_hit_events.write(hit);
    }
}
```

**Почему это плохо:**
- ❌ `unsafe` код (mutable static)
- ❌ Tight coupling (Godot code знает про ECS events)
- ❌ Race conditions (если projectiles обрабатываются параллельно)
- ❌ Сложно тестировать (global state)
- ❌ NODE_TO_ENTITY дублирует VisualRegistry

---

## ✅ Целевая архитектура

**Принципы:**
1. ✅ Godot projectiles = purely tactical (collision detection only)
2. ✅ ECS systems = strategic (damage calculation, death)
3. ✅ Events = единственный способ коммуникации Godot ↔ ECS
4. ✅ No global state (используем Bevy Resources + NonSend)

**Flow (event-driven):**
```text
1. ECS: WeaponFired event
   ↓
2. Godot: weapon_fire_main_thread() → spawn GodotProjectile
   ↓
3. Godot: GodotProjectile::physics_process() → move_and_collide()
   ↓
4. Godot: collision → store hit info IN projectile (не в queue!)
   ↓
5. Godot: projectile_collision_system() → read collisions from projectiles
   ↓
6. Godot: generate ProjectileHit event (EventWriter)
   ↓
7. ECS: process_projectile_hits() → apply damage
```

**Ключевое отличие:** Collision info хранится В projectile, а не в global queue.

---

## 📝 План implementation

### **Фаза 1: Clean up GodotProjectile (убрать global state)**

**1.1. Modify GodotProjectile struct** (`projectile.rs`)

Добавить поле для хранения collision info:
```rust
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct GodotProjectile {
    base: Base<CharacterBody3D>,

    // Existing fields
    pub shooter: Entity,
    pub direction: Vector3,
    pub speed: f32,
    pub damage: u32,
    pub lifetime: f32,

    // ✅ NEW: collision info (хранится в projectile, не в queue)
    pub collision_info: Option<ProjectileCollisionInfo>,
}

/// Collision info (хранится в projectile до обработки ECS)
#[derive(Clone, Debug)]
pub struct ProjectileCollisionInfo {
    pub target_instance_id: InstanceId,
    pub impact_point: Vector3,
    pub impact_normal: Vector3,  // ✅ Для VFX (spark direction, shield ripple, decals)
}
```

**1.2. Modify physics_process() — store collision, don't queue**

```rust
fn physics_process(&mut self, delta: f64) {
    // 1. Move projectile
    let velocity = self.direction * self.speed;
    self.base_mut().set_velocity(velocity);
    let collision = self.base_mut().move_and_collide(velocity * delta as f32);

    // 2. Store collision info (НЕ пушим в queue!)
    if let Some(collision_info) = collision {
        if let Some(collider_node) = collision_info.get_collider() {
            let instance_id = collider_node.instance_id();
            let normal = collision_info.get_normal();  // ✅ Get normal from KinematicCollision3D

            // ✅ Store collision info IN projectile
            self.collision_info = Some(ProjectileCollisionInfo {
                target_instance_id: instance_id,
                impact_point: self.base().get_global_position(),
                impact_normal: normal,  // ✅ For VFX
            });

            voidrun_simulation::log(&format!(
                "🎯 Projectile stored collision: instance_id={:?}, normal={:?}",
                instance_id, normal
            ));

            // NOTE: Projectile НЕ удаляется здесь! ECS система удалит после обработки.
        }
    }

    // 3. Lifetime tick
    self.lifetime -= delta as f32;
    if self.lifetime <= 0.0 {
        self.base_mut().queue_free();
    }
}
```

**Ключевое изменение:** Collision info хранится В projectile, а не в global queue.

---

### **Фаза 2: ECS projectile tracking (Resource)**

**2.1. Create GodotProjectileRegistry** (новый Resource)

Create `crates/voidrun_godot/src/projectile_registry.rs`:
```rust
//! GodotProjectileRegistry — track Godot-managed projectiles for collision processing

use godot::prelude::*;
use std::collections::HashMap;
use crate::projectile::GodotProjectile;

/// Registry для Godot projectiles
///
/// Хранит ссылки на GodotProjectile nodes для collision processing.
/// Обновляется каждый frame (добавляем new projectiles, удаляем destroyed).
#[derive(Default)]
pub struct GodotProjectileRegistry {
    /// InstanceId → GodotProjectile node
    pub projectiles: HashMap<InstanceId, Gd<GodotProjectile>>,
}

impl GodotProjectileRegistry {
    pub fn register(&mut self, projectile: Gd<GodotProjectile>) {
        let instance_id = projectile.instance_id();
        self.projectiles.insert(instance_id, projectile);
        voidrun_simulation::log(&format!("📋 Registered projectile: {:?}", instance_id));
    }

    pub fn unregister(&mut self, instance_id: InstanceId) {
        self.projectiles.remove(&instance_id);
        voidrun_simulation::log(&format!("🗑️ Unregistered projectile: {:?}", instance_id));
    }

    /// Cleanup destroyed projectiles (call every frame)
    pub fn cleanup_destroyed(&mut self) {
        self.projectiles.retain(|id, proj| {
            let is_valid = proj.is_instance_valid();
            if !is_valid {
                voidrun_simulation::log(&format!("🗑️ Cleanup destroyed projectile: {:?}", id));
            }
            is_valid
        });
    }
}
```

**2.2. Register GodotProjectileRegistry** (в SimulationBridge)

Modify `simulation_bridge/mod.rs`:
```rust
use crate::projectile_registry::GodotProjectileRegistry;

// In setup systems (after VisualRegistry):
app.insert_non_send_resource(GodotProjectileRegistry::default());
```

**2.3. Export module**

Modify `lib.rs`:
```rust
pub mod projectile;
pub mod projectile_registry;  // ✅ NEW
```

---

### **Фаза 3: Collision processing system (Godot → ECS events)**

**3.1. Create projectile_collision_system** (новая система)

Modify `systems/weapon_system.rs`:
```rust
use crate::projectile_registry::GodotProjectileRegistry;

/// System: Process projectile collisions (Godot → ECS)
///
/// Reads collision info from GodotProjectile nodes.
/// Generates ProjectileHit events для ECS damage processing.
/// Despawns projectiles after processing.
///
/// **Frequency:** Every frame (60 Hz)
pub fn projectile_collision_system_main_thread(
    mut registry: NonSendMut<GodotProjectileRegistry>,
    visuals: NonSend<VisualRegistry>,
    mut projectile_hit_events: EventWriter<voidrun_simulation::combat::ProjectileHit>,
) {
    // Cleanup destroyed projectiles first
    registry.cleanup_destroyed();

    // Process collisions
    let mut to_remove = Vec::new();

    for (instance_id, mut projectile) in registry.projectiles.iter_mut() {
        // Check if projectile has collision info
        let Some(collision_info) = projectile.bind().collision_info.clone() else {
            continue;  // No collision yet
        };

        // Reverse lookup: InstanceId → Entity
        let Some(&target_entity) = visuals.node_to_entity.get(&collision_info.target_instance_id) else {
            voidrun_simulation::log(&format!(
                "⚠️ Projectile collision with unknown entity (InstanceId: {:?})",
                collision_info.target_instance_id
            ));
            to_remove.push(*instance_id);
            projectile.queue_free();
            continue;
        };

        // Check self-hit (projectile не должна попадать в shooter)
        let shooter = projectile.bind().shooter;
        if target_entity == shooter {
            voidrun_simulation::log(&format!(
                "🚫 Projectile ignored self-collision: shooter={:?}",
                shooter
            ));
            // Clear collision info, projectile продолжает лететь
            projectile.bind_mut().collision_info = None;
            continue;
        }

        // ✅ Generate ProjectileHit event (Godot → ECS)
        let damage = projectile.bind().damage;
        projectile_hit_events.write(voidrun_simulation::combat::ProjectileHit {
            shooter,
            target: target_entity,
            damage,
        });

        voidrun_simulation::log(&format!(
            "💥 Projectile hit! Shooter: {:?} → Target: {:?}, Damage: {} (normal: {:?})",
            shooter, target_entity, damage, collision_info.impact_normal
        ));

        // Despawn projectile
        to_remove.push(*instance_id);
        projectile.queue_free();
    }

    // Cleanup processed projectiles from registry
    for instance_id in to_remove {
        registry.unregister(instance_id);
    }
}
```

---

### **Фаза 4: Update weapon_fire_main_thread (register projectiles)**

**4.1. Modify spawn_godot_projectile()** (`weapon_system.rs`)

```rust
/// Helper: создать GodotProjectile (полностью Godot-managed)
fn spawn_godot_projectile(
    shooter: Entity,
    position: Vector3,
    direction: Vector3,
    speed: f32,
    damage: u32,
    scene_root: &Gd<Node3D>,
    registry: &mut GodotProjectileRegistry,  // ✅ NEW parameter
) {
    use crate::projectile::GodotProjectile;

    // 1. Создаём GodotProjectile node
    let mut projectile = Gd::<GodotProjectile>::from_init_fn(|base| {
        GodotProjectile::init(base)
    });

    projectile.set_position(position);

    // Collision layers: Projectiles (layer 4)
    // Collision mask: Actors + Environment (projectiles hit actors and walls)
    projectile.set_collision_layer(crate::collision_layers::COLLISION_LAYER_PROJECTILES);
    projectile.set_collision_mask(crate::collision_layers::COLLISION_MASK_PROJECTILES);

    // Debug: проверяем что layers установлены
    voidrun_simulation::log(&format!(
        "Projectile collision setup: layer={} mask={}",
        projectile.get_collision_layer(),
        projectile.get_collision_mask()
    ));

    // 2. Setup параметры projectile
    projectile.bind_mut().setup(
        shooter.to_bits() as i64,
        direction,
        speed,
        damage as i64,
    );

    // 3. SphereMesh визуал (красная пуля)
    let mut mesh_instance = godot::classes::MeshInstance3D::new_alloc();
    let mut sphere = SphereMesh::new_gd();
    sphere.set_radius(0.1); // 10 см пуля
    sphere.set_height(0.2);
    mesh_instance.set_mesh(&sphere.upcast::<Mesh>());

    // Красный материал
    let mut material = StandardMaterial3D::new_gd();
    material.set_albedo(Color::from_rgb(1.0, 0.3, 0.3));
    mesh_instance.set_surface_override_material(0, &material.upcast::<Material>());

    projectile.add_child(&mesh_instance.upcast::<Node>());

    // 4. CollisionShape3D (сфера)
    let mut collision = CollisionShape3D::new_alloc();
    let mut sphere_shape = SphereShape3D::new_gd();
    sphere_shape.set_radius(0.1);
    collision.set_shape(&sphere_shape.upcast::<godot::classes::Shape3D>());

    projectile.add_child(&collision.upcast::<Node>());

    // ✅ 5. Register projectile in registry (BEFORE adding to scene)
    registry.register(projectile.clone());

    // 6. Добавляем в сцену (Godot автоматически вызовет _physics_process)
    scene_root.clone().upcast::<Node>().add_child(&projectile.upcast::<Node>());
}
```

**4.2. Update weapon_fire_main_thread()** (pass registry)

```rust
/// System: Process WeaponFired events → spawn Godot projectile
/// Создаёт GodotProjectile (полностью Godot-managed, НЕ в ECS)
/// Direction рассчитывается из weapon bone rotation (+Z forward axis)
///
/// ВАЖНО: Fallback direction использует Godot Transform из VisualRegistry!
pub fn weapon_fire_main_thread(
    mut fire_events: EventReader<WeaponFired>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
    mut registry: NonSendMut<GodotProjectileRegistry>,  // ✅ NEW
) {
    for event in fire_events.read() {
        // Находим actor node
        let Some(actor_node) = visuals.visuals.get(&event.shooter) else {
            voidrun_simulation::log(&format!("Actor {:?} visual not found", event.shooter));
            continue;
        };

        // 1. Находим BulletSpawn node для spawn_position (Golden Path helper)
        let (spawn_position, weapon_node) = find_bullet_spawn_position(actor_node);

        // 2. Рассчитываем direction из weapon bone rotation
        let direction = if let Some(weapon) = weapon_node {
            // Берём +Z axis weapon bone (наша модель смотрит в +Z, не -Z как Godot convention)
            let global_transform = weapon.get_global_transform();
            let dir = global_transform.basis.col_c();
            voidrun_simulation::log(&format!("🔫 Weapon direction: {:?}", dir));
            dir // basis.z = forward для нашей модели
        } else {
            // Fallback: направление от shooter к target (если есть target)
            if let Some(target_entity) = event.target {
                if let Some(target_node) = visuals.visuals.get(&target_entity) {
                    let shooter_pos = actor_node.get_global_position();
                    let target_pos = target_node.get_global_position();
                    (target_pos - shooter_pos).normalized()
                } else {
                    voidrun_simulation::log("Target visual not found, using default forward");
                    Vector3::new(0.0, 0.0, -1.0) // Default -Z forward
                }
            } else {
                // No target (player FPS) → default forward
                Vector3::new(0.0, 0.0, -1.0) // Default -Z forward
            }
        };

        // 3. Создаём GodotProjectile (полностью Godot-managed)
        spawn_godot_projectile(
            event.shooter,
            spawn_position,
            direction,
            event.speed,
            event.damage,
            &scene_root.node,
            &mut registry,  // ✅ Pass registry
        );

        voidrun_simulation::log(&format!(
            "Spawned projectile: shooter={:?} → target={:?} at {:?} dir={:?} dmg={}",
            event.shooter, event.target, spawn_position, direction, event.damage
        ));
    }
}
```

---

### **Фаза 5: Remove old systems (cleanup)**

**5.1. Delete old global state** (`projectile.rs`)

```rust
// ❌ DELETE these lines:
static mut PROJECTILE_HIT_QUEUE: Option<Vec<ProjectileHit>> = None;
static mut NODE_TO_ENTITY: Option<HashMap<InstanceId, Entity>> = None;

pub fn init_projectile_hit_queue() { /* DELETE entire function */ }
pub fn register_collision_body(...) { /* DELETE entire function */ }
pub fn take_projectile_hits() { /* DELETE entire function */ }
```

**5.2. Delete old system** (`weapon_system.rs`)

```rust
// ❌ DELETE this entire system:
pub fn process_godot_projectile_hits(
    mut projectile_hit_events: EventWriter<voidrun_simulation::combat::ProjectileHit>,
) {
    let hits = crate::projectile::take_projectile_hits();
    for hit in hits {
        projectile_hit_events.write(hit);
    }
}
```

**5.3. Update systems registration** (`simulation_bridge/systems_setup.rs`)

```rust
// ❌ REMOVE this line:
app.add_systems(Update, weapon_system::process_godot_projectile_hits.in_set(GodotUpdateSet::MainThread));

// ✅ ADD this line (в том же месте):
app.add_systems(Update, weapon_system::projectile_collision_system_main_thread.in_set(GodotUpdateSet::MainThread));
```

**5.4. Remove init call** (`simulation_bridge/mod.rs`)

```rust
// ❌ DELETE this line:
crate::projectile::init_projectile_hit_queue();
```

Search for any calls to `register_collision_body()` — удалить ВСЕ (VisualRegistry уже трекает nodes).

---

## 🔧 Файлы для модификации

**Core refactoring:**
1. ✅ `crates/voidrun_godot/src/projectile.rs` — remove global state, add collision_info field
2. ✅ `crates/voidrun_godot/src/projectile_registry.rs` — NEW FILE (GodotProjectileRegistry)
3. ✅ `crates/voidrun_godot/src/systems/weapon_system.rs` — new collision system, update spawn
4. ✅ `crates/voidrun_godot/src/simulation_bridge/systems_setup.rs` — update system registration
5. ✅ `crates/voidrun_godot/src/simulation_bridge/mod.rs` — register resource, remove init calls
6. ✅ `crates/voidrun_godot/src/lib.rs` — export projectile_registry module

---

## ✅ Критерии готовности

**Functionality:**
- [ ] Projectiles spawn correctly (визуальная проверка)
- [ ] Projectiles collide with actors (damage applied)
- [ ] Self-hit ignored (shooter не получает урон от своего projectile)
- [ ] Projectiles despawn after hit (no duplicates)
- [ ] Projectiles despawn after lifetime (5s timeout)

**Architecture:**
- [ ] No `static mut` global state (все unsafe удалены)
- [ ] No PROJECTILE_HIT_QUEUE
- [ ] No NODE_TO_ENTITY duplication (используем VisualRegistry)
- [ ] GodotProjectileRegistry управляет lifecycle
- [ ] Event-driven flow (Godot → ECS через Bevy events)
- [ ] impact_normal доступен для VFX (shield ripple, sparks, decals)

**Tests:**
- [ ] `cargo clippy` — no warnings
- [ ] `cargo test` — все тесты проходят
- [ ] Manual testing: 10 NPC стреляют друг в друга (damage работает)
- [ ] Manual testing: Player стреляет → враг получает урон

---

## 📊 Оценка времени

**Фаза 1 (GodotProjectile cleanup):** 1-1.5 часа
- Modify struct + CollisionInfo: 15 мин
- Modify physics_process (+ impact_normal): 30 мин
- Testing: 30 мин

**Фаза 2 (Registry):** 1 час
- Create projectile_registry.rs: 30 мин
- Register resource + exports: 15 мин
- Testing: 15 мин

**Фаза 3 (Collision system):** 1.5-2 часа
- Write projectile_collision_system: 1 час
- Integration: 30 мин
- Testing: 30 мин

**Фаза 4 (Update spawn):** 30 мин
- Modify spawn function: 15 мин
- Pass registry: 15 мин

**Фаза 5 (Cleanup):** 30 мин
- Delete old code: 15 мин
- Update registrations: 15 мин

**Итого:** 4.5-5.5 часов (полдня работы)

---

## 🎯 После рефакторинга

**Готовность к Shield System:**
- ✅ Event-driven projectiles (легко добавить ProjectileShieldHit event)
- ✅ Clean architecture (нет global state)
- ✅ Registry pattern (можем track shield spheres аналогично)
- ✅ impact_normal для shield ripple VFX

**Улучшения:**
- ✅ No unsafe code (безопаснее)
- ✅ No race conditions (Bevy handles thread safety)
- ✅ Testable (можем mock events)
- ✅ Extensible (легко добавить новые projectile types)

---

**Следующий шаг:** Shield System Implementation (см. `shield-system-implementation.md`)

**Версия:** 1.1 (добавлен impact_normal для VFX)
**Последнее обновление:** 2025-10-25
**Статус:** Ready to implement
