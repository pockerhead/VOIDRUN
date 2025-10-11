# Session Log: Navigation & Movement System Fix
**Date:** 2025-01-10
**Duration:** ~3 hours
**Status:** ✅ Resolved

## Problem Statement

AI patrol система с `NavigationAgent3D` работала некорректно:
- Актор двигался всё быстрее и быстрее (экспоненциальное ускорение)
- Через несколько секунд скорость достигала 80+ м/с (вместо 2 м/с)
- Velocity накапливалась между фреймами вместо перезаписи

## Root Cause Analysis

### Неправильная архитектура TSCN префаба:

**Проблемная структура:**
```
Node3D (actor_node)                    ← Root node (визуальный контейнер)
├── Head (MeshInstance3D)
├── Torso (MeshInstance3D)
└── CollisionBody (CharacterBody3D)    ← Physics body как child node
    └── CollisionShape3D
```

**Код движения (ошибочный):**
```rust
// 1. Двигаем child physics body
body.set_velocity(velocity_target);
body.move_and_slide();  // → body двигается на velocity * delta

// 2. Синхронизируем parent node с child body
actor_node.set_global_position(body.get_global_position());  // ← ВОТ ПРОБЛЕМА!
```

### Feedback Loop механизм:

1. **Frame 1:** `body.move_and_slide()` → body двигается на `velocity * delta`
2. **Sync:** `actor_node.set_global_position(body.global_position)` → parent Node3D двигается
3. **Side effect:** Child body двигается **ЕЩЁ РАЗ** (local transform сохраняется относительно parent!)
4. **Frame 2:** `body.get_velocity()` содержит **удвоенную скорость** от двойного движения
5. **Result:** Экспоненциальный рост velocity → актор улетает в космос 🚀

### Подтверждение через логи:

```
[Movement] velocity BEFORE set: Vector3 { x: 0.0, y: 0.0, z: 0.0 }
[Movement] velocity AFTER move_and_slide: Vector3 { x: -0.005892, y: 0.0, z: -0.099727 }

// Следующий frame (40ms спустя):
[Movement] velocity BEFORE set: Vector3 { x: -0.005896, y: 0.0, z: -0.099781 }  ← НЕ сбросилась!
```

`move_and_slide()` изменяет velocity внутри (collision response, sliding), и это значение **переносится в следующий фрейм** из-за feedback loop.

## Solution

### 1. Исправлена структура TSCN префаба:

**Правильная структура (как в 3d-rpg):**
```
CharacterBody3D (actor_node = root)    ← Root node САМ является physics body
├── CollisionShape3D
├── Head (MeshInstance3D)
├── Torso (MeshInstance3D)
└── RightHand (MeshInstance3D)
```

### 2. Изменён код движения:

**До:**
```rust
let Some(mut actor_node) = visuals.visuals.get(&entity).cloned() else { continue; };
let Some(mut body) = actor_node.try_get_node_as::<CharacterBody3D>("CollisionBody") else { continue; };

body.set_velocity(velocity_target);
body.move_and_slide();
actor_node.set_global_position(body.get_global_position());  // ← Убрали!
```

**После:**
```rust
let Some(actor_node) = visuals.visuals.get(&entity).cloned() else { continue; };
let mut body = actor_node.cast::<CharacterBody3D>();  // Root сам CharacterBody3D

let velocity = Vector3::new(
    local_direction.x * MOVE_SPEED,
    body.get_velocity().y,  // Сохраняем гравитацию
    local_direction.z * MOVE_SPEED,
);

body.set_velocity(velocity);  // Полная перезапись
body.move_and_slide();        // Двигает всё дерево сразу
// НЕТ синхронизации — root node сам является physics body!
```

### 3. Обновлён visual_sync.rs:

**До:**
```rust
if let Some(collision_body) = actor_node.try_get_node_as::<Node>("CollisionBody") {
    let collision_id = collision_body.instance_id();
    visuals.node_to_entity.insert(collision_id, entity);
    crate::projectile::register_collision_body(collision_id, entity);
}
```

**После:**
```rust
// actor_node теперь САМ CharacterBody3D
let actor_id = actor_node.instance_id();
crate::projectile::register_collision_body(actor_id, entity);
```

## Files Changed

1. **`godot/actors/test_actor.tscn`**
   - Changed root node: `Node3D` → `CharacterBody3D`
   - Removed child `CollisionBody` node
   - Moved `CollisionShape3D` как прямой child root node

2. **`crates/voidrun_godot/src/systems/movement_system.rs`**
   - Removed `try_get_node_as::<CharacterBody3D>("CollisionBody")`
   - Added `actor_node.cast::<CharacterBody3D>()`
   - Removed `actor_node.set_global_position()` sync
   - Fixed velocity to preserve Y component (gravity)

3. **`crates/voidrun_godot/src/systems/visual_sync.rs`**
   - Removed `CollisionBody` child node lookup
   - Register `actor_node.instance_id()` directly for projectile collisions

## Testing & Verification

**Before fix:**
```
[02:07:39.242] current: (-2.89, 0.37, -2.89) → next: (9.43, 0.5, -5.37) (dist: 12.57m)
[02:07:39.311] current: (2.99, 0.22, -4.07) → next: (9.43, 0.5, -5.37) (dist: 6.57m)
```
**Result:** 6 метров за 69ms = **86 м/с** (вместо 2 м/с)

**After fix:**
- ✅ Скорость стабильная: 2.0 м/с
- ✅ Патрулирование корректное
- ✅ NavigationAgent3D работает
- ✅ Gravity работает
- ✅ Никакого накопления velocity

## Lessons Learned

### 1. **CharacterBody3D ВСЕГДА должен быть root node**

**❌ НЕПРАВИЛЬНО:**
```
Node3D (root)
└── CharacterBody3D (child)
```

**✅ ПРАВИЛЬНО:**
```
CharacterBody3D (root)
├── визуальные mesh'ы
└── CollisionShape3D
```

**Почему:** Godot physics движет CharacterBody3D через `move_and_slide()`. Если он child node, то движение parent создаёт feedback loop.

### 2. **НИКОГДА не синхронизировать parent node с child physics body**

```rust
// ❌ ПЛОХО:
body.move_and_slide();
actor_node.set_global_position(body.get_global_position());  // Feedback loop!

// ✅ ХОРОШО:
body.move_and_slide();  // body сам root node, всё двигается вместе
```

### 3. **Godot hierarchy != Unity/Unreal hierarchy**

В Unity/Unreal обычная практика:
```
GameObject (transform)
└── Rigidbody (physics component)
```

В Godot это **антипаттерн** — physics node **должен быть root**, не component.

### 4. **Проверять референсные проекты перед архитектурными решениями**

3d-rpg проект уже имел правильную структуру:
```gdscript
[node name="Enemy" type="CharacterBody3D"]  # Root = physics body
```

**Вывод:** При возникновении проблем — сначала проверить working reference, не изобретать велосипед.

### 5. **Логирование velocity помогает диагностировать проблемы**

Добавив лог `velocity BEFORE set` сразу увидели накопление:
```rust
let old_velocity = body.get_velocity();
log(&format!("velocity BEFORE: {:?}", old_velocity));
```

### 6. **`move_and_slide()` изменяет velocity внутри**

`move_and_slide()` не просто двигает body — он **модифицирует velocity** (collision response, sliding по стенам). Нужно **полностью перезаписывать velocity каждый фрейм**, не полагаться на старое значение.

## Related Documentation

- **ADR-004:** Command/Event Architecture (Bevy Events)
- **ADR-005:** Transform Ownership (Godot Transform + ECS StrategicPosition)
- **Architecture:** `docs/architecture/godot-rust-integration.md`

## Future Improvements

1. ~~Переключить на `velocity_computed` callback~~ — не нужно (avoidance отключён для single-player)
2. Добавить unit tests для movement system (verify no velocity accumulation)
3. Документировать CharacterBody3D best practices в `docs/architecture/`
4. Проверить weapon attachment систему — возможно аналогичная проблема

## Conclusion

Проблема решена полностью путём исправления архитектуры TSCN префаба. Ключевой урок: **в Godot physics node должен быть root node**, не child wrapper'а. Синхронизация parent ↔ child physics body создаёт feedback loop с экспоненциальным ростом velocity.

**Time to resolution:** 3 часа (большая часть на диагностику через логи)
**Final result:** ✅ Stable 2.0 m/s movement, correct pathfinding
