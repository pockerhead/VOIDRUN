# Camera System & VATS: Hybrid First-Person Combat

**Статус:** Дизайн-документ
**Версия:** 1.0
**Дата:** 2025-01-13

---

## Обзор

Камерная система VOIDRUN использует гибридный подход: **первое лицо как основа + тактическая камера для особых моментов**. Это создаёт баланс между hardcore skill-based gameplay и зрелищными cinematic моментами.

**Философия дизайна:**
- **Core gameplay:** First-person (tension, immersion, skill expression)
- **Special moments:** Third-person tactical view (VATS, finishers, dialogue)
- **Переключение камеры = reward** за хороший gameplay

**Отличие от других игр:**
- **NOT** Fallout (full pause VATS) — у нас slow-mo, не pause
- **NOT** God of War (QTE spam) — finishers редкие, как reward
- **NOT** Mass Effect (fixed dialogue camera) — процедурная, киношная

---

## Camera Modes

### First-Person (Основной режим)

**Когда активен:**
- Исследование, бой, stealth — 90% игрового времени
- Default режим после spawn

**Характеристики:**
```rust
FirstPerson {
    fov: 90.0-110.0,           // Field of view (настраиваемо)
    head_bob: true,            // Покачивание при ходьбе
    weapon_sway: true,         // Inertia оружия
    position: attached_to_head, // Camera = head bone
}
```

**Design принципы:**
- **Hardcore feel:** Ограниченное поле зрения (не видишь за спиной)
- **Skill-based:** Aim полностью manual, нет aim assist (PC)
- **Tension:** Не видишь своё тело → больше уязвимость

**Взаимодействие:**
- Weapon occupies screen space (реалистично, не floating gun)
- Melee weapon visible в покое (на поясе/за спиной)
- Damage indicators at screen edges (не в центре)

### Third-Person Tactical (VATS Mode)

**Когда активен:**
- Player активирует VATS (TAB key)
- Melee finisher trigger
- Debug mode (toggle V)

**Характеристики:**
```rust
ThirdPersonTactical {
    distance: 3.0,             // Метры от персонажа
    angle: Vec3::new(0, 2, -3), // Over-shoulder + высота
    target_focus: Some(Entity), // Кого держим в фокусе
    orbit_speed: 2.0,          // Скорость orbit (mouse)
}
```

**Camera behavior:**
- **Smooth transition** от first-person (0.5 сек)
- **Dynamic positioning** — избегает стены (SpringArm collision)
- **Target focus** — всегда держит target в кадре
- **Player control** — можно orbit мышкой (опционально)

**Visual feedback:**
- Player model visible (чтобы видеть своё состояние)
- Targeting reticles на врагах
- UI overlay (AP bar, target list)

### Cinematic (Finishers & Dialogue)

**Когда активен:**
- Melee finisher execution
- Important dialogue moments
- Scripted story events

**Характеристики:**
```rust
Cinematic {
    shot_type: CinematicShot, // Predefined или procedural
    duration: f32,             // Auto-return после timeout
    allow_skip: bool,          // ESC to skip
    letterbox: bool,           // Черные полосы сверху/снизу
}

pub enum CinematicShot {
    // Dialogue
    OverShoulderPlayer,
    OverShoulderNPC,
    CloseUpPlayer,
    CloseUpNPC,
    TwoShot,

    // Finisher
    FinisherDynamic { weapon_type: WeaponType },
    Killcam,
}
```

**Camera control:**
- **Automated** — player не контролирует
- **Rule of Eight** для dialogue (180° rule)
- **Dynamic angles** для finishers (best viewpoint)
- **Smooth cuts** между shots (0.2-0.5 сек fade)

### Spectator (Debug)

**Когда активен:**
- Debug mode только (не в production)
- Admin/GM режим (в multiplayer)

**Характеристики:**
```rust
Spectator {
    position: Vec3,
    rotation: Quat,
    speed: 10.0,              // WASD movement speed
    turbo_multiplier: 3.0,    // Shift = faster
}
```

**Использование:**
- Freecam для testing
- Cinematic camera preview
- Level design verification

---

## VATS System: Tactical Time Dilation

### Концепция

**Вдохновение:** Fallout 3 VATS + Superhot time mechanics

**Ключевое отличие от Fallout:**
- **Slow-mo (не pause):** Время замедляется до 0.1x, но НЕ останавливается полностью
- **Player двигается:** Можешь уклоняться, repositioning во время targeting
- **Enemies реагируют:** Медленно, но могут стрелять/двигаться
- **Multiplayer-ready:** Нет full pause → можно адаптировать под co-op

### Activation Flow

**1. Pre-activation check:**
```rust
fn can_activate_vats(player: &Player) -> bool {
    player.action_points.current >= VATS_MIN_AP  // Минимум 25 AP
    && !player.is_exhausted()                     // Не в exhaustion
    && !player.is_in_dialogue()                   // Не в диалоге
    && player.vats_cooldown <= 0.0                // Cooldown готов
}
```

**2. Activation (TAB key):**
```
Player нажимает TAB
→ Time dilation: 1.0x → 0.1x (плавно, 0.3 сек)
→ Camera: First-person → Third-person Tactical (0.5 сек)
→ UI: VATS overlay появляется
→ Input mode: Targeting mode (mouse = select targets)
```

**3. Targeting Phase:**
```
Player кликает на врагов (ЛКМ)
→ Raycast от camera к world
→ Hit enemy → add to queue
→ UI показывает:
   - Hit chance % (per body part)
   - Damage preview
   - AP cost
   - Total AP spent

Player может:
- Select до 5 targets (max queue size)
- Cycle body parts (колесо мыши)
- Remove from queue (ПКМ)
- Cancel (ESC → return to normal)
- Execute (ENTER → start execution)
```

**4. Execution Phase:**
```
Player жмёт ENTER
→ Time dilation: 0.1x → 0.5x (боевая скорость)
→ Camera: Follows action (cinematic angles)
→ Player auto-executes queued shots:
   - Aim at target #1
   - Fire (hit chance roll)
   - Slow-mo на kill (0.2x, 0.5 сек)
   - Next target

→ После всех shots:
   - Time dilation: 0.5x → 1.0x
   - Camera: Third-person → First-person
   - VATS cooldown starts (10-30 сек)
```

### Action Points System

**Resource management:**
```rust
#[derive(Component)]
pub struct ActionPoints {
    pub current: f32,
    pub max: f32,
    pub regen_rate: f32,      // AP/sec вне боя
    pub combat_regen: f32,    // AP/sec в бою (меньше)
}

impl Default for ActionPoints {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
            regen_rate: 10.0,     // 10 AP/sec → full за 10 сек
            combat_regen: 2.0,    // 2 AP/sec в бою
        }
    }
}
```

**AP Costs:**
```rust
// VATS shot costs
const VATS_SHOT_BASE: f32 = 20.0;       // Базовая стоимость выстрела
const VATS_SHOT_DISTANCE_MULT: f32 = 0.5; // +0.5 AP за метр

// Body part modifiers
const HEAD_AP_MULT: f32 = 1.5;          // +50% AP за headshot
const TORSO_AP_MULT: f32 = 1.0;         // Без модификатора
const LIMB_AP_MULT: f32 = 1.2;          // +20% AP за limb

// Melee
const VATS_MELEE_COST: f32 = 40.0;      // Дороже чем ranged

// Calculation
fn calculate_vats_cost(
    distance: f32,
    body_part: BodyPart,
) -> f32 {
    let base = VATS_SHOT_BASE;
    let distance_cost = distance * VATS_SHOT_DISTANCE_MULT;
    let part_mult = match body_part {
        BodyPart::Head => HEAD_AP_MULT,
        BodyPart::Torso => TORSO_AP_MULT,
        _ => LIMB_AP_MULT,
    };

    (base + distance_cost) * part_mult
}

// Пример: headshot на 10м = (20 + 10*0.5) * 1.5 = 37.5 AP
// Пример: torso на 5м = (20 + 5*0.5) * 1.0 = 22.5 AP
```

**Regen rules:**
```rust
fn regen_action_points(
    mut players: Query<(&mut ActionPoints, &AIState)>,
    time: Res<Time>,
) {
    for (mut ap, state) in players.iter_mut() {
        let regen = match state {
            AIState::Combat { .. } => ap.combat_regen, // Медленная регенерация
            _ => ap.regen_rate,                         // Быстрая регенерация
        };

        ap.current = (ap.current + regen * time.delta_secs()).min(ap.max);
    }
}
```

### Targeting System

**Body Parts:**
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyPart {
    Head,       // Критический урон, низкий hit chance
    Torso,      // Стандартный урон, высокий hit chance
    LeftArm,    // Disarm эффект, средний hit chance
    RightArm,   // Disarm эффект, средний hit chance
    LeftLeg,    // Slow эффект, средний hit chance
    RightLeg,   // Slow эффект, средний hit chance
}
```

**Hit Chance Calculation:**
```rust
fn calculate_hit_chance(
    shooter: &Actor,
    target: &Actor,
    body_part: BodyPart,
    distance: f32,
    weapon: &Weapon,
) -> f32 {
    // 1. Base chance по body part
    let base_chance = match body_part {
        BodyPart::Head => 0.5,      // 50% base
        BodyPart::Torso => 0.85,    // 85% base
        BodyPart::LeftArm | BodyPart::RightArm => 0.6, // 60%
        BodyPart::LeftLeg | BodyPart::RightLeg => 0.65, // 65%
    };

    // 2. Distance penalty
    let optimal_range = weapon.optimal_range; // 10-15м для rifles
    let distance_mult = if distance <= optimal_range {
        1.0 // Нет penalty
    } else {
        1.0 - ((distance - optimal_range) / weapon.max_range).min(0.5)
        // Max -50% на max_range
    };

    // 3. Weapon accuracy
    let weapon_accuracy = weapon.accuracy; // 0.8-1.0 для хорошего оружия

    // 4. Player skill
    let skill_bonus = shooter.weapon_skill * 0.002; // 0-20% от 0-100 skill

    // 5. Target movement penalty (даже в slow-mo)
    let movement_penalty = target.velocity.length() * 0.1; // -10% за 1 м/с

    // Final calculation
    let final_chance = base_chance
        * distance_mult
        * weapon_accuracy
        + skill_bonus
        - movement_penalty;

    final_chance.clamp(0.05, 0.95) // Минимум 5%, максимум 95%
}
```

**Damage Preview:**
```rust
fn calculate_damage_preview(
    weapon: &Weapon,
    body_part: BodyPart,
) -> u32 {
    let base_damage = weapon.damage;
    let part_mult = match body_part {
        BodyPart::Head => 2.0,      // x2 урон (критический)
        BodyPart::Torso => 1.0,     // x1 урон (стандартный)
        _ => 0.7,                   // x0.7 урон (limbs)
    };

    (base_damage as f32 * part_mult) as u32
}
```

**Target Queue:**
```rust
#[derive(Component)]
pub struct VATSQueue {
    pub targets: Vec<VATSTarget>,
    pub max_size: usize, // 5
}

#[derive(Debug, Clone)]
pub struct VATSTarget {
    pub entity: Entity,
    pub body_part: BodyPart,
    pub hit_chance: f32,
    pub damage: u32,
    pub ap_cost: f32,
}
```

### Time Dilation

**Implementation:**
```rust
#[derive(Resource)]
pub struct TimeDilation {
    pub current_scale: f32,    // Текущий scale (0.0-1.0)
    pub target_scale: f32,     // Target scale (плавно lerp к этому)
    pub transition_speed: f32, // Скорость transition (обычно 2.0-5.0)
}

// VATS activation
time_dilation.target_scale = 0.1; // 10% скорость

// Execution phase
time_dilation.target_scale = 0.5; // 50% скорость

// Return to normal
time_dilation.target_scale = 1.0; // 100% скорость

// Update system (FixedUpdate)
fn update_time_dilation(
    mut time_dilation: ResMut<TimeDilation>,
    time: Res<Time>,
) {
    let delta = time.delta_secs();
    let speed = time_dilation.transition_speed;

    time_dilation.current_scale = lerp(
        time_dilation.current_scale,
        time_dilation.target_scale,
        delta * speed
    );

    // Apply to Bevy Time (если возможно) или custom delta scaling
}
```

**Что замедляется:**
- ✅ Player movement
- ✅ Enemy movement
- ✅ Projectiles
- ✅ Physics
- ✅ AI decisions (реже думают)

**Что НЕ замедляется:**
- ❌ Camera movement (responsive control)
- ❌ UI animations
- ❌ Particle systems (опционально, для красоты)
- ❌ Audio pitch (distorted audio = optional)

### Execution Phase

**Sequential shooting:**
```rust
fn execute_vats_queue(
    mut queue: ResMut<VATSQueue>,
    mut shooter: Query<&Transform, With<Player>>,
    targets: Query<&Transform, With<Actor>>,
    mut commands: Commands,
) {
    for vats_target in queue.targets.drain(..) {
        // 1. Aim at target (rotate player)
        let shooter_transform = shooter.single();
        let target_transform = targets.get(vats_target.entity).unwrap();
        let direction = (target_transform.translation - shooter_transform.translation).normalize();

        // Command: rotate player to face target (smooth, 0.2 сек)
        commands.add(RotateToDirection { entity: shooter_entity, direction, duration: 0.2 });

        // 2. Roll hit chance
        let roll = random::<f32>();
        let hit = roll <= vats_target.hit_chance;

        // 3. Fire weapon
        commands.add(FireWeapon {
            shooter: shooter_entity,
            target: vats_target.entity,
            damage: if hit { vats_target.damage } else { 0 },
        });

        // 4. Camera focus на target (если hit)
        if hit {
            commands.add(CameraFocusTarget { target: vats_target.entity, duration: 0.5 });
        }

        // 5. Slow-mo на kill
        if hit && target_died {
            commands.add(TimeDilationPulse { scale: 0.2, duration: 0.5 });
        }

        // 6. Wait before next shot (0.3-0.5 сек)
        commands.add(Wait { duration: 0.4 });
    }

    // После всех shots → return to normal
    commands.add(ExitVATS);
}
```

**Camera behavior:**
- **Follow action:** Автоматически позиционируется для лучшего view
- **Dynamic cuts:** Быстрые cuts между targets (cinematic)
- **Kill slow-mo:** 0.2x speed на финальный kill, zoom к target
- **Smooth return:** Плавный переход обратно в first-person

---

## Melee Finishers: Cinematic Executions

### Trigger Conditions

**Когда доступен finisher:**
```rust
fn can_trigger_finisher(
    player: &Player,
    enemy: &Actor,
) -> bool {
    // 1. Enemy low HP
    let hp_threshold = enemy.health.max as f32 * 0.2; // 20% HP
    let enemy_low_hp = enemy.health.current as f32 <= hp_threshold;

    // 2. Player в melee range
    let distance = (player.position - enemy.position).length();
    let in_range = distance <= 2.0;

    // 3. Player имеет perk
    let has_perk = player.perks.contains(&Perk::Executioner);

    // 4. Cooldown готов
    let cooldown_ready = player.finisher_cooldown <= 0.0;

    enemy_low_hp && in_range && has_perk && cooldown_ready
}
```

**Prompt window:**
```
Enemy HP < 20%
→ Slow-mo (0.2x) на 1 секунду
→ UI prompt: [E] EXECUTE (1 сек на решение)
→ Player жмёт E → trigger finisher
→ Player игнорирует → return to normal combat
```

### Execution Flow

**1. Freeze & Lock:**
```rust
// Замораживаем оба entities в пространстве
player.lock_position = true;
enemy.lock_position = true;
enemy.lock_rotation = true;

// Позиционируем для sync animation
align_for_finisher(player, enemy, weapon_type);
```

**2. Camera Transition:**
```rust
// Third-person cinematic angle
let camera_offset = calculate_finisher_camera(
    player.position,
    enemy.position,
    weapon_type
);

// Плавный переход (0.3 сек)
camera.transition_to(
    CameraMode::Cinematic {
        position: player.position + camera_offset,
        look_at: enemy.position + Vec3::new(0, 1, 0), // Центр масс
        duration: 0.3,
    }
);
```

**3. Animation Playback:**
```rust
// Синхронизированные animations
match weapon_type {
    WeaponType::Sword => {
        player.play_animation("finisher_sword_thrust");
        enemy.play_animation("finisher_sword_receive");
        // 2.5 сек duration, sync points на frame 30 (stab moment)
    }
    WeaponType::Axe => {
        player.play_animation("finisher_axe_overhead");
        enemy.play_animation("finisher_axe_receive");
    }
    WeaponType::Knife => {
        player.play_animation("finisher_knife_neck");
        enemy.play_animation("finisher_knife_receive");
    }
    WeaponType::Unarmed => {
        player.play_animation("finisher_unarmed_snap");
        enemy.play_animation("finisher_unarmed_receive");
    }
}

// Time dilation during animation
time_dilation = 0.7; // Чуть замедленно, но не слишком

// Slow-mo на момент kill (sync point)
at_sync_point(30) {
    time_dilation = 0.3; // Резкое замедление на 0.5 сек
}
```

**4. VFX & SFX:**
```rust
// Blood spray (на sync point)
spawn_vfx("blood_spray", enemy.position + impact_offset);

// Screen shake (небольшой)
camera.add_trauma(0.3);

// Sound (muffled во время slow-mo)
play_sound("finisher_impact", pitch: 0.7);
```

**5. Return:**
```rust
// После animation завершилась
enemy.mark_for_death(); // Мгновенная смерть (не через damage)

// Camera transition обратно
camera.transition_to(CameraMode::FirstPerson, duration: 0.5);

// Time dilation return
time_dilation.target = 1.0;

// Cooldown
player.finisher_cooldown = 30.0; // 30 сек

// Unlock positions
player.lock_position = false;
```

### Camera Positioning (Procedural)

**Finisher camera rules:**
```rust
fn calculate_finisher_camera(
    player_pos: Vec3,
    enemy_pos: Vec3,
    weapon_type: WeaponType,
) -> Vec3 {
    // Direction от enemy к player
    let to_player = (player_pos - enemy_pos).normalize();

    // Базовая позиция: сбоку + высота
    let side_offset = Vec3::new(to_player.z, 0, -to_player.x) * 2.0; // Перпендикуляр
    let height_offset = Vec3::new(0, 1.5, 0);
    let distance_offset = -to_player * 1.5; // Чуть назад от enemy

    let base_position = enemy_pos + side_offset + height_offset + distance_offset;

    // Raycast check для collision
    if raycast_hits_wall(base_position, enemy_pos) {
        // Fallback: front camera (менее cinematic, но работает)
        enemy_pos - to_player * 2.5 + Vec3::new(0, 1.2, 0)
    } else {
        base_position
    }
}
```

**Weapon-specific adjustments:**
- **Sword/Knife:** Side camera (показывает thrust motion)
- **Axe:** High angle camera (overhead swing лучше видно сверху)
- **Unarmed:** Close-up (emotion + brutality)

### Animation Sync

**Проблема:** Player и enemy animations должны синхронизироваться по времени и пространству.

**Решение 1: Fixed-duration animations**
```rust
// Все finisher animations = 2.0 секунды
// Sync point на frame 60 (из 120 frames @ 60 FPS)
const FINISHER_DURATION: f32 = 2.0;
const FINISHER_SYNC_FRAME: u32 = 60;

// Обе animations начинаются одновременно
player.play_animation_at(0.0);
enemy.play_animation_at(0.0);

// На sync frame = VFX + damage
```

**Решение 2: IK adjustment (advanced)**
```rust
// Inverse Kinematics для динамической подгонки
// Если enemy разного роста → IK adjusts player arm/weapon
// Сложнее, но flexible
```

---

## Dialogue Camera: Cinematic Rule of Eight

### Rule of 180° (Правило восьмёрки)

**Принцип:** Камера не пересекает воображаемую линию между персонажами → spatial continuity.

```
       [NPC]
         ↑
         │
    Line of Action
         │
         ↓
      [Player]

   Camera zone (180°)
     ╱          ╲
   ╱              ╲
  1  2  3  4  5  6  7  ← Допустимые позиции
```

**Почему важно:**
- Если camera jumps через line → зритель теряет ориентацию
- "Кто слева, кто справа?" меняется → confusion
- Cinematic standard (все фильмы используют)

### Shot Types

**1. Over-the-Shoulder (OTS) — Player**
```rust
DialogueShotType::OverShoulderPlayer => {
    // Camera за player плечом, смотрит на NPC
    let to_npc = (npc.position - player.position).normalize();
    let side_offset = Vec3::new(-to_npc.z, 0, to_npc.x) * 0.3; // Сбоку от плеча
    let height_offset = Vec3::new(0, 1.6, 0); // Высота головы

    camera.position = player.position - to_npc * 0.5 + side_offset + height_offset;
    camera.look_at = npc.position + Vec3::new(0, 1.5, 0); // NPC голова
}
```

**Когда использовать:** Player выбирает диалоговую опцию (ты в контроле)

**2. Over-the-Shoulder (OTS) — NPC**
```rust
DialogueShotType::OverShoulderNPC => {
    // Camera за NPC плечом, смотрит на player
    let to_player = (player.position - npc.position).normalize();
    let side_offset = Vec3::new(-to_player.z, 0, to_player.x) * 0.3;
    let height_offset = Vec3::new(0, 1.6, 0);

    camera.position = npc.position - to_player * 0.5 + side_offset + height_offset;
    camera.look_at = player.position + Vec3::new(0, 1.5, 0);
}
```

**Когда использовать:** NPC реагирует на player слова (emotion focus)

**3. Close-Up — Player**
```rust
DialogueShotType::CloseUpPlayer => {
    // Близкий план player лица
    let to_npc = (npc.position - player.position).normalize();

    camera.position = player.position - to_npc * 0.8 + Vec3::new(0, 1.6, 0);
    camera.look_at = player.position + Vec3::new(0, 1.65, 0); // Лицо
    camera.fov = 60.0; // Уже FOV для близкого плана
}
```

**Когда использовать:** Important choice, player угрожает/flirts

**4. Close-Up — NPC**
```rust
DialogueShotType::CloseUpNPC => {
    // Близкий план NPC лица
    let to_player = (player.position - npc.position).normalize();

    camera.position = npc.position - to_player * 0.8 + Vec3::new(0, 1.6, 0);
    camera.look_at = npc.position + Vec3::new(0, 1.65, 0);
    camera.fov = 60.0;
}
```

**Когда использовать:** NPC важное откровение, threat, emotion spike

**5. Two-Shot**
```rust
DialogueShotType::TwoShot => {
    // Оба персонажа в кадре (neutral angle)
    let midpoint = (player.position + npc.position) / 2.0;
    let to_player = (player.position - npc.position).normalize();
    let side_offset = Vec3::new(-to_player.z, 0, to_player.x) * 2.0;

    camera.position = midpoint + side_offset + Vec3::new(0, 1.6, 0);
    camera.look_at = midpoint + Vec3::new(0, 1.5, 0);
    camera.fov = 70.0; // Шире чтобы оба влезли
}
```

**Когда использовать:** Равный диалог, introduction, context establishing

### Shot Transitions

**Cut vs Smooth:**
```rust
match (previous_shot, current_shot) {
    // Cut (instant) для shot changes в dialogue
    (DialogueShotType::OverShoulderPlayer, DialogueShotType::OverShoulderNPC) => {
        camera.cut_to(new_position, new_rotation); // 0 сек
    }

    // Smooth для close-up → two-shot
    (DialogueShotType::CloseUpPlayer, DialogueShotType::TwoShot) => {
        camera.transition_to(new_position, new_rotation, duration: 0.5);
    }

    // Smooth для начала/конца dialogue
    (CameraMode::FirstPerson, DialogueShotType::OverShoulderPlayer) => {
        camera.transition_to(new_position, new_rotation, duration: 0.7);
    }
}
```

**Dialogue Flow Example:**
```
1. Player initiates dialogue (E key)
   → Smooth transition: FirstPerson → OverShoulderPlayer (0.7s)

2. NPC greeting
   → Cut: OverShoulderPlayer → OverShoulderNPC (instant)

3. Player choice shown
   → Cut: OverShoulderNPC → OverShoulderPlayer (instant)

4. Player selects [Intimidate]
   → Cut: OverShoulderPlayer → CloseUpPlayer (instant)
   → Hold 1 second (dramatic pause)

5. NPC reacts (frightened)
   → Cut: CloseUpPlayer → CloseUpNPC (instant)
   → Hold 0.5 second

6. Continue dialogue
   → Cut: CloseUpNPC → OverShoulderNPC (instant)

7. Dialogue ends
   → Smooth: OverShoulderNPC → FirstPerson (0.7s)
```

### Emotion-Driven Camera (Advanced)

**Идея:** Camera angles отражают эмоциональное состояние диалога.

**Low angle (camera снизу):**
- NPC intimidating player
- Player чувствует себя vulnerable
- Creates sense of threat

**High angle (camera сверху):**
- Player intimidating NPC
- NPC pleading/submissive
- Creates sense of power

**Dutch angle (наклон камеры):**
- Unstable situation
- NPC lying/deceptive
- Creates unease

```rust
fn calculate_emotion_camera(
    base_position: Vec3,
    emotion: DialogueEmotion,
) -> Vec3 {
    match emotion {
        DialogueEmotion::Intimidating => {
            base_position + Vec3::new(0, -0.3, 0) // Lower angle
        }
        DialogueEmotion::Submissive => {
            base_position + Vec3::new(0, 0.4, 0) // Higher angle
        }
        DialogueEmotion::Deceptive => {
            // Dutch angle = rotation, not position
            base_position
        }
        DialogueEmotion::Neutral => {
            base_position // Eye level
        }
    }
}
```

---

## Technical Architecture

### Camera Mode State Machine

```rust
#[derive(Component, Debug, Clone, PartialEq)]
pub enum CameraMode {
    FirstPerson {
        fov: f32,
        head_bob: bool,
    },

    ThirdPersonTactical {
        distance: f32,
        angle: Vec3,
        target_focus: Option<Entity>,
    },

    Cinematic {
        shot_type: CinematicShot,
        duration: f32,
        allow_skip: bool,
    },

    Spectator {
        position: Vec3,
        rotation: Quat,
        speed: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CinematicShot {
    // Dialogue
    OverShoulderPlayer,
    OverShoulderNPC,
    CloseUpPlayer,
    CloseUpNPC,
    TwoShot,

    // Finisher
    FinisherDynamic { weapon_type: WeaponType },

    // Story
    Killcam,
    Scripted { event_id: String },
}
```

### Camera Transitions

```rust
#[derive(Component)]
pub struct CameraTransition {
    pub from_mode: CameraMode,
    pub to_mode: CameraMode,
    pub from_position: Vec3,
    pub from_rotation: Quat,
    pub to_position: Vec3,
    pub to_rotation: Quat,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: EasingFunction,
}

pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

fn update_camera_transitions(
    mut query: Query<(&mut Transform, &mut CameraTransition)>,
    time: Res<Time>,
) {
    for (mut transform, mut transition) in query.iter_mut() {
        transition.elapsed += time.delta_secs();

        let t = (transition.elapsed / transition.duration).clamp(0.0, 1.0);
        let eased_t = apply_easing(t, transition.easing);

        // Lerp position
        transform.translation = transition.from_position.lerp(
            transition.to_position,
            eased_t
        );

        // Slerp rotation
        transform.rotation = transition.from_rotation.slerp(
            transition.to_rotation,
            eased_t
        );

        // Transition complete?
        if transition.elapsed >= transition.duration {
            // Remove CameraTransition component
            commands.entity(entity).remove::<CameraTransition>();
        }
    }
}
```

### ECS ↔ Godot Sync

**ECS ответственность:**
- `CameraMode` state (текущий режим)
- `CameraTransition` (если в процессе)
- VATS logic (targeting, AP, queue)
- Finisher triggers

**Godot ответственность:**
- Camera3D positioning (Transform)
- Raycasts (targeting, LOS checks)
- Input handling (mouse look, WASD)
- SpringArm3D (collision avoidance)

**Events (ECS → Godot):**
```rust
#[derive(Event)]
pub enum CameraEvent {
    ModeChanged {
        old_mode: CameraMode,
        new_mode: CameraMode,
        transition_duration: f32,
    },

    FocusTarget {
        target: Entity,
        duration: f32,
    },

    ShakeCamera {
        intensity: f32,
        duration: f32,
    },
}
```

**Events (Godot → ECS):**
```rust
#[derive(Event)]
pub enum GodotCameraEvent {
    TargetSelected {
        entity: Entity,
        body_part: BodyPart,
        hit_chance: f32,
    },

    TransitionComplete {
        mode: CameraMode,
    },
}
```

### Time Dilation Implementation

**Bevy Time Scaling:**
```rust
// Custom time resource (вместо Bevy Time::set_relative_speed)
#[derive(Resource)]
pub struct GameTime {
    pub real_delta: f32,      // Реальное delta (unscaled)
    pub scaled_delta: f32,    // Scaled delta = real_delta * dilation
    pub dilation_scale: f32,  // Current scale (0.0-1.0)
}

fn update_game_time(
    mut game_time: ResMut<GameTime>,
    time: Res<Time>,
    dilation: Res<TimeDilation>,
) {
    game_time.real_delta = time.delta_secs();
    game_time.scaled_delta = game_time.real_delta * dilation.current_scale;
}

// Все gameplay systems используют game_time.scaled_delta
fn example_system(game_time: Res<GameTime>) {
    let delta = game_time.scaled_delta; // Автоматически scaled
    // ...
}
```

**Godot Engine Time:**
```gdscript
# В Godot (если нужно синхронизировать)
Engine.time_scale = time_dilation.current_scale
```

**Exclusions (не замедляются):**
```rust
// Камера uses real_delta
fn camera_movement(
    game_time: Res<GameTime>,
    mut camera: Query<&mut Transform, With<Camera>>,
) {
    let delta = game_time.real_delta; // NOT scaled
    // Camera movement остаётся responsive
}

// UI animations uses real_delta
fn ui_animations(game_time: Res<GameTime>) {
    let delta = game_time.real_delta;
    // ...
}
```

---

## Gameplay Balance

### VATS vs Manual Aim Trade-offs

**VATS Преимущества:**
- ✅ Guaranteed hit (если > 0% chance)
- ✅ Multiple targets быстро
- ✅ Body part targeting (tactical choice)
- ✅ Slow-mo = легче принимать решения
- ✅ Cinematic = cool factor

**VATS Недостатки:**
- ❌ AP cost (ограниченный ресурс)
- ❌ Cooldown 10-30 сек после использования
- ❌ Enemies могут двигаться (slow-mo, не frozen)
- ❌ Time spent в targeting mode = vulnerability
- ❌ Не работает в некоторых ситуациях (stealth?)

**Manual Aim Преимущества:**
- ✅ Нет AP cost
- ✅ Всегда доступен
- ✅ Skill ceiling (headshot master = OP)
- ✅ Быстрее (нет transition delays)
- ✅ Full control

**Manual Aim Недостатки:**
- ❌ Требует skill
- ❌ Стресс (real-time, нет паузы)
- ❌ Multiple targets сложнее
- ❌ No preview (не знаешь hit chance)

**Design Goal:** Оба стиля viable, выбор = playstyle preference

### AP Economy

**Sources (как получить AP):**
1. **Time regen:** 10 AP/sec вне боя, 2 AP/sec в бою
2. **Perks:** +20% max AP, +50% regen rate
3. **Consumables:** Stims, drugs (+50 AP instant)
4. **Gear:** AP regen implants

**Sinks (как тратить AP):**
1. **VATS shots:** 20-40 AP за выстрел
2. **VATS melee:** 40 AP за удар
3. **Special abilities:** Dodge (20 AP), Sprint (5 AP/sec)

**Balance target:**
- VATS доступен каждые 20-30 секунд в бою
- 1 VATS use = 3-5 shots (зависит от distance/body parts)
- Не spam (иначе теряет special feel)

### Cooldowns

**VATS Cooldown:**
```rust
const VATS_COOLDOWN_BASE: f32 = 10.0; // 10 сек base

// Модификаторы
let cooldown = VATS_COOLDOWN_BASE
    * (1.0 + shots_fired * 0.5)  // +50% за каждый shot
    * perk_multiplier;            // Perks могут снижать

// Пример: 5 shots = 10 * (1 + 5*0.5) = 35 сек cooldown
```

**Finisher Cooldown:**
```rust
const FINISHER_COOLDOWN: f32 = 30.0; // 30 сек фиксированно
```

**Design rationale:**
- VATS = tactical tool, не spam
- Finishers = rare reward
- Encourage skilled play между VATS uses

### Skill Expression

**Новичок:**
- Полагается на VATS (guaranteed hits)
- Torso shots (высокий hit chance)
- Простой подход

**Опытный:**
- Mix VATS + manual aim
- VATS для clutch moments (multiple enemies)
- Manual для single target (faster)

**Мастер:**
- Редко использует VATS (только для style)
- Manual headshots (нет AP cost)
- VATS = finisher setup

---

## Implementation Roadmap

### Phase 1: Camera Modes Foundation (3-5 дней)

**Задачи:**
1. `CameraMode` enum + state machine
2. First-person camera:
   - Attach to player head bone
   - FOV 90-110° (настраиваемо)
   - Head bob (опционально)
3. Third-person camera:
   - Orbital positioning (distance, angle)
   - Follow player smoothly
   - SpringArm collision avoidance
4. Camera transitions:
   - `CameraTransition` component
   - Smooth lerp position/rotation
   - Easing functions
5. Input: [V] toggle 1st/3rd person (debug)

**Deliverables:**
- `voidrun_simulation/src/camera/mod.rs` — CameraMode types
- `voidrun_simulation/src/camera/transitions.rs` — transition logic
- `voidrun_godot/src/camera/camera_controller.rs` — Godot sync
- Basic test: switch между 1st/3rd person работает

### Phase 2: VATS Core (5-7 дней)

**Задачи:**
1. Action Points system:
   - `ActionPoints` component
   - Regen logic (combat vs non-combat)
   - UI bar (simple)
2. VATS activation:
   - TAB key → slow-mo (0.1x)
   - Camera: 1st → 3rd person tactical
   - UI overlay (basic)
3. Targeting system:
   - Raycast от camera к world
   - Body part detection (hitboxes)
   - Hit chance calculation
   - Damage preview
4. Target queue:
   - Add/remove targets (mouse)
   - Display queue в UI
   - AP cost preview
5. Execution phase:
   - Sequential shooting
   - Camera follows action
   - Basic slow-mo на kills

**Deliverables:**
- `voidrun_simulation/src/vats/mod.rs` — VATS logic
- `voidrun_simulation/src/vats/targeting.rs` — hit chance, body parts
- `voidrun_godot/src/vats/vats_ui.rs` — UI overlay
- `voidrun_godot/src/vats/execution.rs` — execution phase
- Playable VATS: можно queue shots и execute

### Phase 3: VATS Polish (3-5 дней)

**Задачи:**
1. Cinematic camera angles:
   - Dynamic positioning (best view)
   - Smooth cuts между targets
   - Zoom на kills
2. Time dilation варьирование:
   - 0.1x targeting
   - 0.5x execution
   - 0.2x на kills (pulse)
3. VFX:
   - Bullet trails (visible в slow-mo)
   - Impact effects
   - Target reticles (world-space)
4. SFX:
   - Slow-mo audio distortion (optional)
   - VATS activation sound
   - UI feedback sounds
5. Balance tweaks:
   - AP costs tuning
   - Cooldowns adjustment
   - Hit chance formula refinement

**Deliverables:**
- Polished VATS experience
- VFX/SFX integration
- Balance spreadsheet

### Phase 4: Melee Finishers (5-7 дней)

**Задачи:**
1. Finisher trigger system:
   - Low HP detection (< 20%)
   - Perk check
   - Cooldown management
   - UI prompt ([E] Execute)
2. Finisher animations:
   - 3-5 variants per weapon type
   - Sword, axe, knife, unarmed
   - Fixed duration (2.0 сек)
   - Sync points для VFX
3. Cinematic camera:
   - Procedural positioning
   - Collision avoidance
   - Smooth transition in/out
4. Execution sync:
   - Player + enemy animation sync
   - VFX на sync point (blood, impact)
   - Screen shake
   - Slow-mo timing
5. Polish:
   - Skip option (ESC)
   - Cooldown UI
   - Sound design

**Deliverables:**
- `voidrun_simulation/src/combat/finishers.rs` — finisher logic
- `voidrun_godot/src/combat/finisher_camera.rs` — camera controller
- Finisher animations (placeholder или mocap)
- Working finishers для всех weapon types

### Phase 5: Dialogue Camera (3-5 дней)

**Задачи:**
1. Dialogue system (basic):
   - Dialogue tree structure (simple)
   - Speaker/listener tracking
   - Choice presentation
2. Camera shots:
   - OTS player
   - OTS NPC
   - Close-up player
   - Close-up NPC
   - Two-shot
3. Rule of Eight positioning:
   - Procedural calculation
   - Collision avoidance
   - Smooth transitions vs cuts
4. Shot sequencing:
   - Dialogue flow → camera shots
   - Emotion-driven angles (optional)
   - Timing (hold duration per shot)
5. Polish:
   - Letterbox mode (optional)
   - Skip dialogue (ESC)
   - Subtitles integration

**Deliverables:**
- `voidrun_simulation/src/dialogue/mod.rs` — dialogue system
- `voidrun_godot/src/dialogue/camera.rs` — dialogue camera
- Test dialogue scene (2-3 NPCs)
- Cinematic dialogue работает

---

## Trade-offs & Risks

### Risk 1: Time Dilation в Multiplayer

**Проблема:**
- Slow-mo = проблема для других игроков
- Co-op: один player в VATS → другой в slow-mo (annoying)

**Решения:**

**Option A: VATS = Single-player only**
- В co-op VATS отключен полностью
- Simplest solution
- Минус: теряем feature в multiplayer

**Option B: Local Bullet Time (Matrix style)**
- VATS player видит slow-mo
- Остальные видят нормальную скорость + visual effect на VATS player
- Сложнее технически (desync issues)
- Плюс: сохраняет feature

**Option C: Shared Slow-mo (Superhot-like)**
- Все игроки замедляются вместе
- Требует coordination
- Может быть fun (team VATS combos?)

**Рекомендация:** Option A для Phase 1 (single-player focus), Option B для Phase 2+ (если multiplayer)

### Risk 2: Camera Clipping (Third-person)

**Проблема:**
- 3rd person camera может уходить в стены
- Особенно в узких коридорах

**Решение:**
```rust
// SpringArm3D (Godot built-in)
let mut spring_arm = SpringArm3D::new();
spring_arm.set_length(3.0);          // Желаемая distance
spring_arm.set_collision_mask(1);    // Только walls
spring_arm.set_margin(0.2);          // Buffer distance

// Camera = child of SpringArm
// SpringArm автоматически pulls camera closer если raycast hits wall
```

**Альтернатива:** Fade to first-person если camera слишком близко (< 1м)

### Risk 3: Animation Sync (Finishers)

**Проблема:**
- Player и enemy animations должны sync perfectly
- Разный рост enemies → misalignment

**Решения:**

**Option A: Fixed animations**
- Все enemies одного размера (humanoid)
- Animations бьют точно
- Простота > реализм

**Option B: IK (Inverse Kinematics)**
- Player arm/weapon adjusts к enemy position
- Flexible, но сложнее
- Требует IK solver (Godot поддерживает)

**Option C: Animation variants**
- Separate animations для small/medium/large enemies
- Больше work, но controllable

**Рекомендация:** Option A для Phase 1, Option B для polish

### Risk 4: VATS Balance

**Проблема:**
- VATS может быть слишком OP (trivializes combat)
- Или бесполезен (никто не использует)

**Mitigation:**
```rust
// Playtesting metrics
struct VATSMetrics {
    usage_frequency: f32,     // Сколько раз в минуту
    success_rate: f32,        // % успешных shots
    preference_ratio: f32,    // VATS shots / manual shots
}

// Target balance:
// - usage_frequency: 1-2 раза в минуту (не spam)
// - success_rate: 60-80% (не guaranteed win)
// - preference_ratio: 0.3-0.5 (30-50% shots через VATS)
```

**Tuning levers:**
- AP costs (выше = реже использование)
- Cooldowns (длиннее = реже)
- Hit chance formula (ниже = менее reliable)
- Enemy reactions (могут уклоняться в slow-mo?)

---

## Креативные Идеи (Bonus Features)

### "Blade Mode" (Metal Gear Rising)

**Концепция:** Melee version of VATS

**Механика:**
1. Player активирует Blade Mode (similar to VATS)
2. Time slow-mo (0.1x)
3. Camera: free rotation вокруг enemy (mouse drag)
4. Player выбирает направление slashes (mouse gestures)
5. Execution: серия быстрых ударов с разных углов

**Implementation:**
```rust
// Targeting phase
BladeMode {
    slashes: Vec<SlashDirection>, // Up to 5 slashes
    ap_cost: 50.0,                // Дороже чем gun VATS
}

pub enum SlashDirection {
    Horizontal,
    Vertical,
    DiagonalUpRight,
    DiagonalDownLeft,
    // etc
}
```

**Cinematic:**
- Camera follows каждый slash (quick cuts)
- Slow-mo на финальный удар
- Dismemberment (optional, gore)

### "Bullet Time" Passive Perk

**Концепция:** Automatic slow-mo когда low HP (Last Stand)

**Trigger:**
```rust
if player.health.current < player.health.max * 0.2  // < 20% HP
   && !player.bullet_time_active
   && player.bullet_time_cooldown <= 0.0 {

    // Активируем bullet time
    time_dilation.target = 0.5;  // 50% speed
    player.bullet_time_active = true;
    player.bullet_time_duration = 3.0; // 3 секунды
}
```

**Effect:**
- 3 секунды slow-mo
- Шанс уклониться/контратаковать
- Cooldown 60 сек (один раз за encounter обычно)

**Visual feedback:**
- Screen desaturation (чёрно-белое)
- Vignette (tunnel vision)
- Heartbeat sound

### "Killcam" (Skyrim-style)

**Концепция:** Автоматическая cinematic camera на финальный kill в encounter

**Trigger:**
```rust
if enemy.is_last_in_encounter() && enemy.health <= kill_threshold {
    // Killcam activation
    camera_mode = CameraMode::Cinematic {
        shot_type: CinematicShot::Killcam,
        duration: 2.0,
    };
}
```

**Camera:**
- Third-person (shows player + enemy)
- Slow-mo (0.3x)
- Dynamic angle (best view of kill)
- Epic music sting (short, 2 сек)

**Skip:**
- ESC to skip (return to normal)
- Auto-skip после 2 секунд

---

## Ссылки

**Связанные документы:**
- [docs/design/shield-technology.md](shield-technology.md) — Shield mechanics (влияет на combat balance)
- [docs/architecture/combat-system.md](../architecture/combat-system.md) — Combat architecture
- [docs/roadmap.md](../roadmap.md) — Implementation timeline

**ADRs:**
- ADR-002: Godot-Rust Integration
- ADR-003: Hybrid ECS/Godot Architecture
- ADR-004: Command/Event Architecture
- ADR-009: Camera System & VATS Design (этот документ)

**Inspiration:**
- Fallout 3/4 VATS (targeting, AP system)
- Superhot (time dilation mechanics)
- Max Payne (bullet time feel)
- Metal Gear Rising (Blade Mode)
- The Last of Us (finisher execution quality)
- Uncharted/The Last of Us (dialogue camera)

---

**Последнее обновление:** 2025-01-13
**Автор:** VOIDRUN Design Team
**Статус:** Approved для implementation (Phase 1-2 priority)
