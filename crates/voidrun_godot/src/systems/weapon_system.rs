//! Weapon system - Godot визуализация (aim, fire, projectiles)
//!
//! Architecture (ADR-005):
//! - ECS: Weapon state (cooldown, fire decisions) → WeaponFired events
//! - Godot: Aim execution (bone rotation), Projectile spawn + physics
//! - Events: WeaponFired (ECS→Godot), ProjectileHit (Godot→ECS)

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{Node3D, SphereMesh, StandardMaterial3D, Mesh, Material, CollisionShape3D, SphereShape3D, Node, ICharacterBody3D};
use voidrun_simulation::*;
use voidrun_simulation::combat::{WeaponFired, WeaponFireIntent, AttackType, MeleeAttackState, WeaponStats};
use voidrun_simulation::ai::{GodotAIEvent, SpottedEnemies};
use crate::systems::VisualRegistry;
use crate::actor_utils::{actors_facing_each_other, angles};

// ============================================================================
// Systems: Weapon Aim + Fire
// ============================================================================

/// System: Dynamic target switching (SlowUpdate schedule, 0.3 Hz)
///
/// Для ВСЕХ акторов в AIState::Combat:
/// - Проверяет ближайшего ВИДИМОГО врага из SpottedEnemies (VisionCone + LOS raycast)
/// - Если ближайший враг ≠ текущий target → переключает target
///
/// **Результат:** AI всегда атакует ближайшего видимого врага (dynamic target prioritization)
///
/// **Schedule:** SlowUpdate (0.3 Hz = ~3 раза в секунду)
/// - Экономия CPU (не нужно каждый frame)
/// - Более реалистичное поведение AI (время реакции ~0.3с)
/// - Избегаем "perfect play" эффект (instant target switching)
///
/// ВАЖНО: НЕ зависит от WeaponFireIntent events (отдельная система)
pub fn update_combat_targets_main_thread(
    mut actors: Query<(Entity, &Actor, &mut ai::AIState, &ai::SpottedEnemies), With<Actor>>,
    all_actors: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
) {

    // Получаем PhysicsDirectSpaceState3D один раз для всех акторов
    let world = scene_root.node.get_world_3d();
    let Some(mut world) = world else {
        return;
    };

    let space = world.get_direct_space_state();
    let Some(mut space) = space else {
        return;
    };

    for (entity, actor, mut ai_state, spotted_enemies) in actors.iter_mut() {
        // Обрабатываем только Combat state
        let ai::AIState::Combat { target: current_target } = ai_state.as_ref() else {
            continue;
        };

        // Получаем shooter node для distance calculation
        let Some(shooter_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let shooter_pos = shooter_node.get_global_position();
        let shooter_eye = shooter_pos + Vector3::new(0.0, 0.8, 0.0); // Eye level

        // Ищем БЛИЖАЙШЕГО ВИДИМОГО врага из SpottedEnemies
        let mut closest_visible_enemy: Option<(Entity, f32)> = None;

        for &enemy_entity in &spotted_enemies.enemies {
            // Проверяем что враг жив (есть в actors)
            let Ok(enemy_actor) = all_actors.get(enemy_entity) else {
                continue;
            };

            // Проверяем faction (только враги)
            if enemy_actor.faction_id == actor.faction_id {
                continue;
            }

            // Получаем Godot node для distance + LOS check
            let Some(enemy_node) = visuals.visuals.get(&enemy_entity) else {
                continue;
            };

            let enemy_pos = enemy_node.get_global_position();
            let distance_to_enemy = (enemy_pos - shooter_pos).length();

            // ✅ LOS CHECK: raycast от shooter к enemy (eye-level)
            let enemy_eye = enemy_pos + Vector3::new(0.0, 0.8, 0.0);

            let query_params = godot::classes::PhysicsRayQueryParameters3D::create(shooter_eye, enemy_eye);
            let Some(mut query) = query_params else {
                continue;
            };

            query.set_collision_mask(crate::collision_layers::COLLISION_MASK_RAYCAST_LOS);
            let empty_array = godot::prelude::Array::new();
            query.set_exclude(&empty_array);

            let result = space.intersect_ray(&query);

            // Проверяем результат raycast
            if result.is_empty() {
                // Нет коллизий → странно, skip
                continue;
            }

            let Some(collider_variant) = result.get("collider") else {
                continue;
            };

            let Ok(collider_node) = collider_variant.try_to::<Gd<godot::classes::Node>>() else {
                continue;
            };

            let collider_id = collider_node.instance_id();
            let enemy_instance_id = enemy_node.instance_id();

            // Если попали НЕ в enemy → LOS заблокирован, skip
            if collider_id != enemy_instance_id {
                continue;
            }

            // ✅ ВРАГ ВИДИМ! Обновляем ближайшего
            if let Some((_, current_min_dist)) = closest_visible_enemy {
                if distance_to_enemy < current_min_dist {
                    closest_visible_enemy = Some((enemy_entity, distance_to_enemy));
                }
            } else {
                closest_visible_enemy = Some((enemy_entity, distance_to_enemy));
            }
        }

        // Если нашли ближайшего видимого и он НЕ равен текущему target → переключаем
        if let Some((closest_entity, closest_distance)) = closest_visible_enemy {
            if closest_entity != *current_target {
                // ✅ ЗАМЕНЯЕМ TARGET в AIState::Combat
                if let ai::AIState::Combat { ref mut target } = ai_state.as_mut() {
                    let old_target = *target;
                    *target = closest_entity;

                    voidrun_simulation::log(&format!(
                        "🎯 TARGET SWITCH (closest visible): {:?} switches from {:?} to {:?} at {:.1}m",
                        entity, old_target, closest_entity, closest_distance
                    ));
                }
            }
        }
    }

}

/// System: Aim weapon at target (RightHand rotation)
/// Если актёр в Combat state → поворачиваем руку к target
///
/// ВАЖНО: Использует Godot Transform из VisualRegistry (не ECS Transform!)
pub fn weapon_aim_main_thread(
    actors: Query<(Entity, &ai::AIState), With<Actor>>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, state) in actors.iter() {
        // Целимся только в Combat state
        if let ai::AIState::Combat { target } = state {
            // Получаем actor node (shooter)
            let Some(mut actor_node) = visuals.visuals.get(&entity).cloned() else {
                continue;
            };

            // Получаем target node (НЕ ECS Transform — Godot Transform!)
            let Some(target_node) = visuals.visuals.get(target).cloned() else {
                continue;
            };

            // Godot positions (tactical layer — authoritative для aim)
            let target_pos = target_node.get_global_position();
            let actor_pos = actor_node.get_global_position();
            let to_target = target_pos - actor_pos;

            if to_target.length() > 0.01 {
                // Поворачиваем весь actor body к target
                actor_node.look_at(target_pos);

                // Дополнительно поворачиваем RightHand (оружие) к target для точного прицеливания
                if let Some(mut right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") {
                    right_hand.look_at(target_pos);
                }
            }
        }
    }
}

/// System: Process WeaponFireIntent → validate distance/LOS → generate WeaponFired
///
/// Архитектура (Hybrid Intent-based):
/// - ECS отправил WeaponFireIntent (strategic: "хочу стрелять")
/// - Godot проверяет tactical constraints (distance, line of sight)
/// - Если OK → генерирует WeaponFired для spawn projectile
///
/// **Note:** Target switching обрабатывается отдельной системой `update_combat_targets_main_thread`
///
/// ВАЖНО: Использует Godot Transform из VisualRegistry (authoritative!)
pub fn process_ranged_attack_intents_main_thread(
    mut intent_events: EventReader<WeaponFireIntent>,
    actors: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
    mut fire_events: EventWriter<WeaponFired>,
) {
    for intent in intent_events.read() {
        // Получаем shooter node
        let Some(shooter_node) = visuals.visuals.get(&intent.shooter).cloned() else {
            voidrun_simulation::log(&format!(
                "Weapon intent rejected: shooter {:?} visual not found",
                intent.shooter
            ));
            continue;
        };

        // Player FPS shooting (no target) → skip validation, emit WeaponFired immediately
        let Some(target_entity) = intent.target else {
            fire_events.write(WeaponFired {
                shooter: intent.shooter,
                target: None,
                damage: intent.damage,
                speed: intent.speed,
                shooter_position: {
                    let pos = shooter_node.get_global_position();
                    Vec3::new(pos.x, pos.y, pos.z)
                },
                hearing_range: intent.hearing_range,
            });
            continue;
        };

        // AI shooting (has target) → validate distance + LOS
        let Some(target_node) = visuals.visuals.get(&target_entity).cloned() else {
            voidrun_simulation::log(&format!(
                "Weapon intent rejected: target {:?} visual not found",
                target_entity
            ));
            continue;
        };

        // ✅ Tactical validation: distance check (Godot Transform authoritative)
        let shooter_pos = shooter_node.get_global_position();
        let target_pos = target_node.get_global_position();
        let distance = (target_pos - shooter_pos).length();

        if distance > intent.max_range {
            voidrun_simulation::log(&format!(
                "Weapon intent rejected: distance {:.1}m > max_range {:.1}m (shooter {:?} → target {:?})",
                distance, intent.max_range, intent.shooter, target_entity
            ));
            continue;
        }

        if distance < 0.5 {
            voidrun_simulation::log(&format!(
                "Weapon intent rejected: too close {:.1}m (shooter {:?} → target {:?})",
                distance, intent.shooter, target_entity
            ));
            continue;
        }

        // ✅ Line-of-Sight Check: raycast от shooter к target (eye-level Y+0.8)
        let shooter_eye = shooter_pos + Vector3::new(0.0, 0.8, 0.0);
        let target_eye = target_pos + Vector3::new(0.0, 0.8, 0.0);

        let world = scene_root.node.get_world_3d();
        let Some(mut world) = world else {
            voidrun_simulation::log_error("process_weapon_fire_intents: World3D не найден");
            continue;
        };

        let space = world.get_direct_space_state();
        let Some(mut space) = space else {
            voidrun_simulation::log_error("process_weapon_fire_intents: PhysicsDirectSpaceState3D не найден");
            continue;
        };

        // Создаём raycast query
        let query_params = godot::classes::PhysicsRayQueryParameters3D::create(shooter_eye, target_eye);
        let Some(mut query) = query_params else {
            voidrun_simulation::log_error("process_weapon_fire_intents: PhysicsRayQueryParameters3D::create failed");
            continue;
        };

        // Collision mask: Actors + Environment (LOS check)
        query.set_collision_mask(crate::collision_layers::COLLISION_MASK_RAYCAST_LOS);

        let empty_array = godot::prelude::Array::new();
        query.set_exclude(&empty_array); // Проверяем все коллизии

        // Выполняем raycast
        let result = space.intersect_ray(&query);

        // Проверяем результат
        if result.is_empty() {
            // Нет коллизий → странно (target должен быть виден), НЕ стреляем
            voidrun_simulation::log(&format!(
                "🚫 LOS CHECK FAILED: no raycast hit (shooter {:?} → target {:?}, distance {:.1}m) - possible raycast bug or target out of range",
                intent.shooter, target_entity, distance
            ));
            continue;
        }

        let Some(collider_variant) = result.get("collider") else {
            voidrun_simulation::log_error("process_weapon_fire_intents: raycast result missing 'collider'");
            continue;
        };

        let Ok(collider_node) = collider_variant.try_to::<Gd<godot::classes::Node>>() else {
            voidrun_simulation::log_error("process_weapon_fire_intents: collider не является Node");
            continue;
        };

        let collider_id = collider_node.instance_id();

        // Получаем target node instance_id
        let target_instance_id = target_node.instance_id();

        // Если попали в target → всё OK, продолжаем
        if collider_id == target_instance_id {
            // LOS clear, попали точно в target
        } else {
            // Попали НЕ в target → проверяем что это (стена? союзник? враг?)

            // Пытаемся найти entity по collider instance_id (reverse lookup)
            let Some(&collider_entity) = visuals.node_to_entity.get(&collider_id) else {
                // Не actor → вероятно стена/препятствие (layer 3)
                // LOS blocked → отклоняем fire intent (movement_system обработает)
                voidrun_simulation::log(&format!(
                    "🚫 LOS BLOCKED BY OBSTACLE: shooter {:?} → target {:?} (obstacle: {:?}) - fire intent rejected",
                    intent.shooter, target_entity, collider_id
                ));
                continue;
            };

            // Это actor → проверяем faction
            let Ok(collider_actor) = actors.get(collider_entity) else {
                voidrun_simulation::log(&format!(
                    "⚠️ Collider entity {:?} has no Actor component",
                    collider_entity
                ));
                continue;
            };

            let Ok(shooter_actor) = actors.get(intent.shooter) else {
                continue;
            };

            if collider_actor.faction_id == shooter_actor.faction_id {
                // Союзник на линии огня → НЕ стреляем
                voidrun_simulation::log(&format!(
                    "🚫 FRIENDLY FIRE RISK: shooter {:?} (faction {}) won't shoot through ally {:?} (faction {}) at target {:?}",
                    intent.shooter, shooter_actor.faction_id, collider_entity, collider_actor.faction_id, target_entity
                ));
                continue;
            }

            // Враг на линии огня → НЕ стреляем (target switching обработает update_combat_targets_main_thread)
            voidrun_simulation::log(&format!(
                "🚫 LOS BLOCKED BY ENEMY: shooter {:?} → target {:?} blocked by enemy {:?} (faction {})",
                intent.shooter, target_entity, collider_entity, collider_actor.faction_id
            ));
            continue;
        }

        // ✅ All tactical validations passed → генерируем WeaponFired
        fire_events.write(WeaponFired {
            shooter: intent.shooter,
            target: Some(target_entity),
            damage: intent.damage,
            speed: intent.speed,
            shooter_position: Vec3::new(shooter_pos.x, shooter_pos.y, shooter_pos.z),  // Godot Vector3 → Bevy Vec3
            hearing_range: intent.hearing_range,  // Радиус слышимости из оружия
        });

        voidrun_simulation::log(&format!(
            "Weapon intent APPROVED: shooter {:?} → target {:?} (distance: {:.1}m)",
            intent.shooter, target_entity, distance
        ));
    }
}

/// Helper: Find bullet spawn position (BulletSpawn → weapon root → RightHand → actor)
///
/// Returns: (spawn_position, weapon_node_for_direction)
fn find_bullet_spawn_position(actor_node: &Gd<Node3D>) -> (Vector3, Option<Gd<Node3D>>) {
    // Try 1: RightHandAttachment (attachment point)
    let Some(weapon_attachment) = actor_node.try_get_node_as::<Node3D>("%RightHandAttachment") else {
        // Fallback 1: RightHand
        if let Some(right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") {
            voidrun_simulation::log("⚠️ WeaponAttachment not found, using RightHand");
            return (right_hand.get_global_position(), Some(right_hand));
        }

        // Fallback 2: Actor position
        voidrun_simulation::log("⚠️ RightHand not found, using actor position");
        return (actor_node.get_global_position(), None);
    };

    // Try 2: Get weapon prefab (first child of attachment)
    let weapon_prefab = if weapon_attachment.get_child_count() > 0 {
        weapon_attachment.get_child(0).and_then(|node| node.try_cast::<Node3D>().ok())
    } else {
        None
    };

    let Some(weapon_prefab) = weapon_prefab else {
        voidrun_simulation::log("⚠️ No weapon attached to RightHandAttachment");
        return (weapon_attachment.get_global_position(), Some(weapon_attachment));
    };

    // Try 3: Find BulletSpawn via unique name
    if let Some(bullet_spawn_node) = weapon_prefab.get_node_or_null("%BulletSpawn") {
        if let Ok(bullet_spawn) = bullet_spawn_node.try_cast::<Node3D>() {
            return (bullet_spawn.get_global_position(), Some(bullet_spawn));
        }
    }

    // Try 4: Legacy fallback - recursive search
    if let Some(bullet_spawn) = find_node_recursive(&weapon_attachment, "BulletSpawn") {
        return (bullet_spawn.get_global_position(), Some(bullet_spawn));
    }

    // Fallback 5: Weapon root position
    voidrun_simulation::log("⚠️ BulletSpawn not found (add unique_name_in_owner to weapon prefab)");
    (weapon_prefab.get_global_position(), Some(weapon_prefab))
}

/// System: Process WeaponFired events → spawn Godot projectile
/// Создаёт GodotProjectile (полностью Godot-managed, НЕ в ECS)
/// Direction рассчитывается из weapon bone rotation (+Z forward axis)
///
/// ВАЖНО: Fallback direction использует Godot Transform из VisualRegistry!
pub fn weapon_fire_main_thread(
    mut fire_events: EventReader<WeaponFired>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
    mut registry: NonSendMut<crate::projectile_registry::GodotProjectileRegistry>,
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
            &mut registry,
        );

        voidrun_simulation::log(&format!(
            "Spawned projectile: shooter={:?} → target={:?} at {:?} dir={:?} dmg={}",
            event.shooter, event.target, spawn_position, direction, event.damage
        ));
    }
}

/// Helper: создать GodotProjectile (полностью Godot-managed)
fn spawn_godot_projectile(
    shooter: Entity,
    position: Vector3,
    direction: Vector3,
    speed: f32,
    damage: u32,
    scene_root: &Gd<Node3D>,
    registry: &mut crate::projectile_registry::GodotProjectileRegistry,
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

    // 5. Register projectile in registry (BEFORE adding to scene)
    registry.register(projectile.clone());

    // 6. Добавляем в сцену (Godot автоматически вызовет _physics_process)
    scene_root.clone().upcast::<Node>().add_child(&projectile.upcast::<Node>());
}

// ❌ projectile_physics удалена — GodotProjectile::physics_process обрабатывает всё

/// System: Process projectile collisions (Godot → ECS)
///
/// Reads collision info from GodotProjectile nodes.
/// Generates ProjectileHit events для ECS damage processing.
/// Despawns projectiles after processing.
///
/// **Frequency:** Every frame (60 Hz)
pub fn projectile_collision_system_main_thread(
    mut registry: NonSendMut<crate::projectile_registry::GodotProjectileRegistry>,
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
        };

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

/// Helper: рекурсивный поиск node по имени
fn find_node_recursive(parent: &Gd<Node3D>, name: &str) -> Option<Gd<Node3D>> {
    for i in 0..parent.get_child_count() {
        if let Some(child) = parent.get_child(i) {
            if child.get_name().to_string() == name {
                return child.try_cast::<Node3D>().ok();
            }
            // Рекурсивно ищем в детях
            if let Ok(child_node3d) = child.try_cast::<Node3D>() {
                if let Some(found) = find_node_recursive(&child_node3d, name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// System: Detect visible melee windups (CombatUpdate, 10 Hz)
///
/// For all actors in Windup phase:
/// - Spatial query: enemies within weapon range
/// - Angle check: **MUTUAL FACING** (both attacker→defender AND defender→attacker within 35° cone)
/// - Visibility: defender in attacker's SpottedEnemies
/// - Emit: GodotAIEvent::EnemyWindupVisible (broadcast to all visible defenders)
///
/// **AI реагирует на визуальные cues (реалистично, работает для player + AI)**
///
/// **Frequency:** 10 Hz (CombatUpdate schedule)
/// **Parameters:** Hardcoded (angle 35°, будущий балансинг через WeaponStats)
pub fn detect_melee_windups_main_thread(
    attackers: Query<(Entity, &Actor, &MeleeAttackState, &WeaponStats, &SpottedEnemies)>,
    defenders: Query<&Actor>,
    visuals: NonSend<VisualRegistry>,
    mut ai_events: EventWriter<GodotAIEvent>,
) {
    for (attacker_entity, attacker_actor, attack_state, weapon, spotted) in attackers.iter() {
        // Только Windup phase
        if !attack_state.is_windup() {
            continue;
        }

        // Godot Transform (tactical layer)
        let Some(attacker_node) = visuals.visuals.get(&attacker_entity) else {
            continue;
        };

        let attacker_pos = attacker_node.get_global_position();

        // Spatial query: все видимые враги в spotted
        for &defender_entity in &spotted.enemies {
            // Проверка faction (только враги)
            let Ok(defender_actor) = defenders.get(defender_entity) else {
                continue;
            };

            if defender_actor.faction_id == attacker_actor.faction_id {
                continue;
            }

            // Distance check
            let Some(defender_node) = visuals.visuals.get(&defender_entity) else {
                continue;
            };

            let defender_pos = defender_node.get_global_position();
            let distance = (defender_pos - attacker_pos).length();

            if distance > weapon.attack_radius {
                continue;
            }

            // ✅ MUTUAL FACING CHECK (using actor_utils)
            let Some((dot_attacker, dot_defender)) = actors_facing_each_other(
                attacker_node,
                defender_node,
                angles::TIGHT_35_DEG,
            ) else {
                continue; // Not facing each other
            };

            // ✅ MUTUAL FACING - DEFENDER CAN SEE WINDUP!
            ai_events.write(GodotAIEvent::EnemyWindupVisible {
                attacker: attacker_entity,
                defender: defender_entity,
                attack_type: AttackType::Melee, // Всегда Melee для melee атак
                windup_remaining: attack_state.phase_timer,
            });

            voidrun_simulation::log(&format!(
                "👁️ Windup visible (MUTUAL FACING): {:?} → {:?} (distance: {:.1}m, attacker_angle: {:.2}, defender_angle: {:.2}, windup: {:.2}s)",
                attacker_entity, defender_entity, distance, dot_attacker, dot_defender, attack_state.phase_timer
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_aim_only_in_combat() {
        // Verify aim system only triggers in Combat state
        // (unit test без Godot API)
    }
}
