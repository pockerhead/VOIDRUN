# Shield System Implementation Plan

**Статус:** 📋 Planned (после рефакторинга projectile на event-driven)
**Версия:** 1.0
**Дата:** 2025-10-25
**Оценка:** 2-3 дня

---

## 📋 Обзор

Реализация Energy Shield system согласно design doc `shield-technology.md`. Щит блокирует ranged урон, но пропускает melee атаки (slow kinetic). Включает recharge систему, визуализацию через collision sphere + shader VFX, и 4 модели щитов (Military/Commercial/Civilian/Legacy).

---

## ✅ Что уже есть

**ECS Components (готовы к использованию):**
- ✅ `EnergyShield` component в `components/equipment.rs:248-345`
  - `max_energy`, `current_energy`, `recharge_rate`, `recharge_delay`, `velocity_threshold`
  - `tick()` метод для recharge system
  - Presets: `military()`, `basic()`

**Combat Systems:**
- ✅ `DamageDealt` event в `combat/damage.rs:30-36`
- ✅ `calculate_damage()` функция в `damage.rs:97-114`
- ✅ Melee hit detection в `melee_system.rs:224-290`
- ✅ Ranged projectile system (требует рефакторинга на event-driven)

---

## 🔄 Архитектурные решения

### **1. DamageType enum вместо velocity**

**Было рассмотрено:** `velocity: f32` в `DamageDealt` event для определения типа урона.

**Решение:** `DamageType` enum для явной семантики.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageType {
    Melee,
    Ranged,
    Environmental, // для будущего (огонь, яд, etc)
}

#[derive(Event, Debug, Clone)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: u32,
    pub damage_type: DamageType,  // ✅ Явный тип вместо velocity
    pub applied_damage: AppliedDamage,  // Для визуальных эффектов
}
```

**Плюсы:**
- ✅ Проще логика: `if damage_type == DamageType::Ranged { shield.absorb() }`
- ✅ Не нужно хранить velocity в event (меньше данных)
- ✅ Легко расширять (Environmental, Explosion, Poison)
- ✅ Явная семантика (не магическое число 5.0 м/с)

---

### **2. Shield Collision Sphere (Godot Layer)**

**Архитектура:**
```
Actor (CharacterBody3D)
  └── ShieldSphere (Area3D)  [NEW]
       ├── CollisionShape3D (SphereShape radius=1.5м)
       └── ShieldMesh (MeshInstance3D с shader)
```

**Зачем:**
- ✅ Projectile collision с щитом (физический контакт)
- ✅ VFX shader на mesh (ripple effect при попадании)
- ✅ Визуальная обратная связь (игрок видит где щит)
- ✅ Раньше остановка projectile (на границе щита, не на body)

**Shield Sphere Flow:**
```text
1. Projectile летит
2. Projectile Area3D overlaps с ShieldSphere Area3D
3. Godot система детектит overlap → генерирует ProjectileShieldHit event
4. ECS обрабатывает event → разряжает shield → генерирует DamageDealt
5. Godot despawn projectile + trigger VFX (ripple на shield mesh)
```

---

## 📝 План implementation (3 фазы)

### **Фаза 1: ECS Shield Logic (1 день)**

**Prerequisite:** Projectile система должна быть event-driven (см. отдельный рефакторинг).

**1.1. DamageType enum** (`combat/damage.rs`)

Добавить enum:
```rust
/// Тип урона (определяет взаимодействие со щитом)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DamageType {
    /// Melee weapon hit (игнорирует щит)
    Melee,
    /// Ranged projectile hit (блокируется щитом)
    Ranged,
    /// Environmental damage (огонь, яд - для будущего)
    Environmental,
}

/// Результат применения урона (для визуальных эффектов)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum AppliedDamage {
    /// Щит поглотил весь урон
    ShieldAbsorbed,
    /// Щит пробит, остаток урона прошёл в health
    ShieldBrokenWithOverflow(u32),
    /// Урон прошёл напрямую (melee или щита нет)
    Direct,
}
```

**1.2. Modify DamageDealt event**

```rust
/// Событие: урон нанесен
///
/// Генерируется после применения damage к Health (и щиту если есть).
/// Используется для UI, звуков, эффектов.
#[derive(Event, Debug, Clone)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: u32,
    pub damage_type: DamageType,  // ✅ NEW
    pub applied_damage: AppliedDamage,  // ✅ NEW
}
```

**1.3. Shield damage calculation**

Новая функция в `damage.rs`:
```rust
/// Применить урон с учётом щита
///
/// Shield blocking logic (из shield-technology.md):
/// - Ranged урон → разряжает щит (если активен)
/// - Melee урон → игнорирует щит полностью
/// - Environmental → TODO (пока Direct)
///
/// Returns: AppliedDamage для визуальных эффектов
pub fn apply_damage_with_shield(
    target_health: &mut Health,
    target_shield: Option<&mut EnergyShield>,
    damage: u32,
    damage_type: DamageType,
) -> AppliedDamage {
    if let Some(shield) = target_shield {
        // Только ranged блокируется щитом
        if damage_type == DamageType::Ranged && shield.is_active() {
            let shield_damage = damage as f32;
            shield.take_damage(shield_damage);

            // Check if shield broke (overflow damage goes to health)
            if shield.current_energy <= 0.0 {
                let overflow = (-shield.current_energy) as u32;
                target_health.damage(overflow);
                return AppliedDamage::ShieldBrokenWithOverflow(overflow);
            }

            return AppliedDamage::ShieldAbsorbed;
        }
    }

    // Melee, Environmental, или щита нет → прямой урон
    target_health.damage(damage);
    AppliedDamage::Direct
}
```

**1.4. Update MeleeHit → DamageDealt flow**

Modify `process_melee_hits()` в `combat/melee.rs`:
```rust
// Generate DamageDealt event with DamageType::Melee
for hit in melee_hit_events.read() {
    let Ok((mut target_health, target_shield)) = targets.get_mut(hit.target) else {
        continue;
    };

    let applied = apply_damage_with_shield(
        &mut target_health,
        target_shield,
        hit.damage,
        DamageType::Melee,  // ✅ Melee игнорирует щит
    );

    damage_dealt_events.write(DamageDealt {
        attacker: hit.attacker,
        target: hit.target,
        damage: hit.damage,
        damage_type: DamageType::Melee,
        applied_damage: applied,
    });
}
```

**1.5. Update ProjectileHit → DamageDealt flow**

Modify projectile hit processing (после рефакторинга на event-driven):
```rust
// Предполагаем что после рефакторинга есть ProjectileHit event
for hit in projectile_hit_events.read() {
    let Ok((mut target_health, target_shield)) = targets.get_mut(hit.target) else {
        continue;
    };

    let applied = apply_damage_with_shield(
        &mut target_health,
        target_shield,
        hit.damage,
        DamageType::Ranged,  // ✅ Projectile = ranged
    );

    damage_dealt_events.write(DamageDealt {
        attacker: hit.attacker,
        target: hit.target,
        damage: hit.damage,
        damage_type: DamageType::Ranged,
        applied_damage: applied,
    });
}
```

**1.6. Shield recharge system**

Новая ECS система:
```rust
/// System: Shield recharge (вне боя)
///
/// Tick shield energy regeneration после recharge_delay.
/// Runs in FixedUpdate (64 Hz).
pub fn shield_recharge_system(
    mut shields: Query<&mut EnergyShield>,
    time: Res<Time>,
) {
    for mut shield in shields.iter_mut() {
        shield.tick(time.delta_secs());
    }
}
```

Добавить в `SimulationPlugin`:
```rust
app.add_systems(FixedUpdate, shield_recharge_system);
```

**Тесты (Фаза 1):**
- ✅ `test_ranged_damage_absorbed_by_shield` — ranged урон разряжает щит
- ✅ `test_melee_damage_ignores_shield` — melee урон игнорирует щит
- ✅ `test_shield_overflow_damage` — overflow урон идёт в health
- ✅ `test_shield_recharge_after_delay` — recharge работает
- ✅ `test_damage_type_enum` — enum работает корректно

---

### **Фаза 2: Shield Collision Sphere (Godot Layer) (0.5 дня)**

**2.1. ShieldSphere prefab** (Godot TSCN)

Modify `godot/actors/test_actor.tscn`:
```
Actor (CharacterBody3D)
  ├── [existing nodes...]
  └── ShieldSphere (Area3D)  [NEW]
       ├── CollisionShape3D (SphereShape radius=1.5м)
       │   └── shape: SphereShape3D { radius: 1.5 }
       └── ShieldMesh (MeshInstance3D)
            ├── mesh: SphereMesh { radius: 1.5, height: 3.0 }
            └── material_override: ShaderMaterial
                 └── shader: res://shaders/shield_shader.gdshader
```

**Collision layers:**
```gdscript
# ShieldSphere (Area3D)
collision_layer = 0b0000_0100  # Layer 3: SHIELD
collision_mask  = 0b0000_1000  # Mask 4: PROJECTILE (детектит projectiles)
```

**2.2. Shield shader** (новый файл)

Create `godot/shaders/shield_shader.gdshader`:
```gdshader
shader_type spatial;
render_mode blend_add, cull_back, depth_draw_opaque, unshaded;

// Uniforms (устанавливаются из Rust)
uniform vec3 shield_color : source_color = vec3(0.3, 0.6, 1.0);
uniform float energy_percent : hint_range(0.0, 1.0) = 1.0;
uniform vec3 last_hit_pos = vec3(0.0, 0.0, 0.0);
uniform float last_hit_time = -999.0;

void fragment() {
    // Fresnel effect (края ярче центра)
    vec3 view_dir = normalize(VIEW);
    vec3 normal = normalize(NORMAL);
    float fresnel = pow(1.0 - abs(dot(normal, view_dir)), 3.0);

    // Ripple effect от last_hit_pos
    vec3 world_pos = (INV_VIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
    float dist_to_hit = distance(world_pos, last_hit_pos);
    float time_since_hit = TIME - last_hit_time;

    float ripple = 0.0;
    if (time_since_hit < 0.5) {  // 0.5s после попадания
        float wave = sin(dist_to_hit * 10.0 - time_since_hit * 20.0);
        float attenuation = exp(-dist_to_hit * 2.0) * (1.0 - time_since_hit * 2.0);
        ripple = wave * attenuation;
    }

    // Alpha fade based on energy
    float base_alpha = fresnel * energy_percent * 0.3;
    float final_alpha = base_alpha + ripple * 0.5;

    ALBEDO = shield_color;
    ALPHA = final_alpha;
    EMISSION = shield_color * fresnel * 2.0;
}
```

**2.3. Projectile → ShieldSphere collision system** (новая система)

Create `systems/projectile_shield_system.rs`:
```rust
/// System: Detect projectile collisions with shield spheres
///
/// Checks Area3D overlaps между projectiles и ShieldSphere.
/// Генерирует ProjectileShieldHit event когда overlap detected.
///
/// Flow:
/// 1. Poll projectile Area3D.get_overlapping_areas()
/// 2. Check if overlapping area = ShieldSphere (reverse lookup)
/// 3. Check if target has active EnergyShield
/// 4. Generate ProjectileShieldHit event
/// 5. Despawn projectile
pub fn projectile_shield_collision_main_thread(
    projectiles: Query<(Entity, &Projectile)>,
    shields: Query<&EnergyShield>,
    visuals: NonSend<VisualRegistry>,
    mut commands: Commands,
    mut shield_hit_events: EventWriter<ProjectileShieldHit>,
) {
    for (proj_entity, projectile) in projectiles.iter() {
        let Some(proj_node) = visuals.visuals.get(&proj_entity) else {
            continue;
        };

        // Get projectile Area3D
        let Some(proj_area) = proj_node.try_get_node_as::<Area3D>("ProjectileArea") else {
            continue;
        };

        // Check overlaps with ShieldSphere areas
        let overlapping = proj_area.get_overlapping_areas();
        for i in 0..overlapping.len() {
            let Some(area) = overlapping.get(i) else { continue };

            // Reverse lookup: Godot Area → ECS Entity
            let instance_id = area.instance_id();
            let Some(&target_entity) = visuals.node_to_entity.get(&instance_id) else {
                continue;
            };

            // Check if target has active shield
            let Ok(shield) = shields.get(target_entity) else {
                continue;
            };

            if !shield.is_active() {
                continue;  // Shield depleted, projectile passes through
            }

            // Generate ProjectileShieldHit event
            let impact_point = proj_node.get_global_position();
            shield_hit_events.write(ProjectileShieldHit {
                projectile: proj_entity,
                target: target_entity,
                damage: projectile.damage,
                impact_point: Vector3::new(
                    impact_point.x,
                    impact_point.y,
                    impact_point.z,
                ),
            });

            // Despawn projectile (absorbed by shield)
            commands.entity(proj_entity).despawn();

            voidrun_simulation::log(&format!(
                "🛡️ Projectile {:?} hit shield of {:?}, absorbed",
                proj_entity, target_entity
            ));

            break;  // One projectile can only hit one shield
        }
    }
}
```

**2.4. Shield VFX update system** (новая система)

```rust
/// System: Update shield shader uniforms on hit
///
/// Listens to ProjectileShieldHit events.
/// Updates ShaderMaterial uniforms (last_hit_pos, last_hit_time).
pub fn update_shield_vfx_on_hit_main_thread(
    mut shield_hit_events: EventReader<ProjectileShieldHit>,
    shields: Query<&EnergyShield>,
    visuals: NonSend<VisualRegistry>,
    time: Res<Time>,
) {
    for hit in shield_hit_events.read() {
        let Some(target_node) = visuals.visuals.get(&hit.target) else {
            continue;
        };

        // Get ShieldMesh
        let Some(shield_sphere) = target_node.try_get_node_as::<Node3D>("ShieldSphere") else {
            continue;
        };
        let Some(mut shield_mesh) = shield_sphere.try_get_node_as::<MeshInstance3D>("ShieldMesh") else {
            continue;
        };

        // Get ShaderMaterial
        let Some(mut material) = shield_mesh.get_material_override() else {
            continue;
        };
        let mut shader_mat = material.cast::<ShaderMaterial>();

        // Update shader uniforms
        shader_mat.set_shader_parameter(
            "last_hit_pos".into(),
            Variant::from(hit.impact_point),
        );
        shader_mat.set_shader_parameter(
            "last_hit_time".into(),
            Variant::from(time.elapsed_secs()),
        );

        voidrun_simulation::log(&format!(
            "✨ Shield VFX triggered at {:?}",
            hit.impact_point
        ));
    }
}
```

**2.5. Shield energy visualization** (update existing system)

Modify `update_ui_labels_main_thread()` в `visual_sync.rs`:
```rust
// Update shader uniform for energy_percent
if let Ok(shield) = shields.get(entity) {
    let energy_percent = shield.current_energy / shield.max_energy;

    // Update ShieldMesh material
    if let Some(shield_mesh) = /* get ShieldMesh */ {
        shader_mat.set_shader_parameter(
            "energy_percent".into(),
            Variant::from(energy_percent),
        );
    }

    // Update text label
    label.text += &format!(" [Shield: {:.0}/{:.0}]",
        shield.current_energy,
        shield.max_energy
    );
}
```

---

### **Фаза 3: Balance & Models (0.5 дня)**

**3.1. Shield models** (расширить `EnergyShield` impl)

В `components/equipment.rs`:
```rust
impl EnergyShield {
    /// Military-grade shield (лучший)
    pub fn military() -> Self {
        Self::new(500.0, 20.0, 2.0)  // Уже есть
    }

    /// Commercial shield (стандартный)
    pub fn commercial() -> Self {
        Self::new(350.0, 15.0, 2.5)
    }

    /// Civilian shield (слабый, без auto-recharge)
    pub fn civilian() -> Self {
        Self {
            max_energy: 200.0,
            current_energy: 200.0,
            recharge_rate: 0.0,  // No auto-recharge (нужна ручная перезарядка)
            recharge_delay: 0.0,
            velocity_threshold: 5.0,
            recharge_timer: 0.0,
        }
    }

    /// Legacy shield (устаревший, нестабильный)
    pub fn legacy() -> Self {
        Self::new(150.0, 5.0, 4.0)  // Медленная регенерация
    }
}
```

**3.2. Spawn shields on actors**

Modify `spawn_actor()` в `simulation_bridge/mod.rs`:
```rust
// Add EnergyShield component (50% of NPCs)
if actor_type == ActorType::Player {
    commands.entity(actor_entity).insert(EnergyShield::military());
} else if rand::random::<f32>() < 0.5 {
    // 50% NPC spawn с базовым щитом
    commands.entity(actor_entity).insert(EnergyShield::basic());
}
```

**3.3. Balance tests** (integration tests)

Create `tests/shield_balance.rs`:
```rust
#[test]
fn test_ranged_vs_shielded_target() {
    // Setup: attacker (ranged weapon) + defender (shield)
    // Fire projectile → verify shield depletes
    // Fire until shield breaks → verify health damage after break
}

#[test]
fn test_melee_vs_shielded_target() {
    // Setup: attacker (melee weapon) + defender (shield)
    // Melee attack → verify shield ignored, health damaged immediately
}

#[test]
fn test_shield_recharge() {
    // Setup: shield takes damage
    // Wait recharge_delay → verify energy starts regenerating
    // Wait full recharge → verify energy = max_energy
}

#[test]
fn test_shield_overflow_damage() {
    // Setup: shield with 50 energy, incoming 100 damage
    // Hit → verify shield = 0, health -= 50 (overflow)
}
```

**3.4. Manual testing checklist**
- [ ] Ranged projectile hits shield → синий ripple effect
- [ ] Melee attack ignores shield → урон напрямую
- [ ] Shield depletes → прозрачность увеличивается
- [ ] Shield breaks → красные частицы + overflow урон
- [ ] Shield recharges → energy bar растёт

---

## 🔧 Файлы для модификации

**ECS Layer (voidrun_simulation):**
1. `crates/voidrun_simulation/src/combat/damage.rs` — DamageType enum, shield logic
2. `crates/voidrun_simulation/src/combat/melee.rs` — melee DamageType::Melee
3. `crates/voidrun_simulation/src/combat/weapon.rs` — ranged DamageType::Ranged (после рефакторинга)
4. `crates/voidrun_simulation/src/lib.rs` — register shield_recharge_system

**Godot Layer (voidrun_godot):**
1. `crates/voidrun_godot/src/systems/projectile_shield_system.rs` — NEW FILE (collision detection)
2. `crates/voidrun_godot/src/systems/visual_sync.rs` — shield energy UI update
3. `crates/voidrun_godot/src/simulation_bridge/mod.rs` — spawn shields
4. `crates/voidrun_godot/src/systems/mod.rs` — register new systems

**Godot Assets:**
1. `godot/actors/test_actor.tscn` — add ShieldSphere node
2. `godot/shaders/shield_shader.gdshader` — NEW FILE (shield VFX)

**Tests:**
1. `tests/shield_balance.rs` — NEW FILE (integration tests)

---

## ✅ Критерии готовности

**Functionality:**
- [ ] Ranged урон разряжает щит
- [ ] Melee урон игнорирует щит
- [ ] Shield overflow урон идёт в health
- [ ] Shield recharge работает (delay + regen)
- [ ] 4 модели щитов (Military/Commercial/Civilian/Legacy)
- [ ] Projectile collision с ShieldSphere (не с body)

**Visualization:**
- [ ] Shield sphere видна (синее мерцание)
- [ ] Shield ripple effect при попадании
- [ ] Shield energy fade (прозрачность зависит от заряда)
- [ ] Shield broken VFX (красные частицы + explosion)
- [ ] Shield stats в UI label

**Balance:**
- [ ] Military shield: 500 energy, 20/sec regen (танки выживают 5+ hits)
- [ ] Commercial shield: 350 energy, 15/sec regen
- [ ] Civilian shield: 200 energy, no auto-recharge
- [ ] Legacy shield: 150 energy, медленная regen (5/sec)

**Tests:**
- [ ] `cargo test shield` — все тесты проходят
- [ ] 10 NPC (5 с щитами, 5 без) в бою @ 60+ FPS
- [ ] Визуальная проверка в Godot (sphere + VFX работают)

---

## 📊 Оценка времени

**Фаза 1 (ECS Logic):** 6-8 часов
- DamageType enum + refactor: 2h
- Shield damage calculation: 2h
- Recharge system: 1h
- Unit tests: 1-2h
- Integration tests: 1-2h

**Фаза 2 (Collision Sphere + VFX):** 4-5 часов
- ShieldSphere TSCN prefab: 1h
- Shield shader (Godot): 1.5h
- Projectile collision system: 1.5h
- VFX update system: 1h

**Фаза 3 (Balance):** 2-3 часа
- Shield models: 0.5h
- Spawn integration: 0.5h
- Balance tests: 1h
- Manual testing & polish: 1h

**Итого:** 12-16 часов (1.5-2 дня coding, 0.5 дня testing/polish)

---

## 🚧 Prerequisites

**КРИТИЧНО:** Перед началом shield implementation нужно завершить:
- ✅ Рефакторинг projectile системы на event-driven (вместо PROJECTILE_HITQUEUE)
- ✅ Projectile collision detection должна генерировать events

**После рефакторинга projectile:**
- Будет `ProjectileHit` event
- Можно добавить `ProjectileShieldHit` event
- Легко интегрировать shield logic

---

## 🎯 После реализации

**Shield System будет полностью функционален:**
- ✅ Ranged vs Melee balance работает
- ✅ Тактическая глубина (hybrid attacks эффективны)
- ✅ Визуализация понятна игрокам
- ✅ 4 модели щитов для variety

**Готовность к следующей фазе:**
- Player HUD polish (crosshair, ammo, shield indicator)
- Chunk system (procedural generation)
- Campaign system (если захочется narrative)

---

**Версия:** 1.0
**Последнее обновление:** 2025-10-25
**Статус:** Ждёт рефакторинга projectile системы
