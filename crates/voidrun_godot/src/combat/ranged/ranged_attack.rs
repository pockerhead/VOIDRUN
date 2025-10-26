//! Ranged attack processing: intent validation and projectile spawning.

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::{Node3D, Node, SphereMesh, StandardMaterial3D, Mesh, Material, CollisionShape3D, SphereShape3D};
use voidrun_simulation::*;
use voidrun_simulation::combat::{WeaponFired, WeaponFireIntent};
use crate::shared::VisualRegistry;
use voidrun_simulation::logger;
// ============================================================================
// Systems: Ranged Attack Processing
// ============================================================================

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
    scene_root: NonSend<crate::shared::SceneRoot>,
    mut fire_events: EventWriter<WeaponFired>,
) {
    for intent in intent_events.read() {
        // Получаем shooter node
        let Some(shooter_node) = visuals.visuals.get(&intent.shooter).cloned() else {
            logger::log(&format!(
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
            logger::log(&format!(
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
            logger::log(&format!(
                "Weapon intent rejected: distance {:.1}m > max_range {:.1}m (shooter {:?} → target {:?})",
                distance, intent.max_range, intent.shooter, target_entity
            ));
            continue;
        }

        if distance < 0.5 {
            logger::log(&format!(
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
            logger::log_error("process_weapon_fire_intents: World3D не найден");
            continue;
        };

        let space = world.get_direct_space_state();
        let Some(mut space) = space else {
            logger::log_error("process_weapon_fire_intents: PhysicsDirectSpaceState3D не найден");
            continue;
        };

        // Создаём raycast query
        let query_params = godot::classes::PhysicsRayQueryParameters3D::create(shooter_eye, target_eye);
        let Some(mut query) = query_params else {
            logger::log_error("process_weapon_fire_intents: PhysicsRayQueryParameters3D::create failed");
            continue;
        };

        // Collision mask: Actors + Environment (LOS check)
        query.set_collision_mask(crate::shared::collision::COLLISION_MASK_RAYCAST_LOS);

        let empty_array = godot::prelude::Array::new();
        query.set_exclude(&empty_array); // Проверяем все коллизии

        // Выполняем raycast
        let result = space.intersect_ray(&query);

        // Проверяем результат
        if result.is_empty() {
            // Нет коллизий → странно (target должен быть виден), НЕ стреляем
            logger::log(&format!(
                "🚫 LOS CHECK FAILED: no raycast hit (shooter {:?} → target {:?}, distance {:.1}m) - possible raycast bug or target out of range",
                intent.shooter, target_entity, distance
            ));
            continue;
        }

        let Some(collider_variant) = result.get("collider") else {
            logger::log_error("process_weapon_fire_intents: raycast result missing 'collider'");
            continue;
        };

        let Ok(collider_node) = collider_variant.try_to::<Gd<godot::classes::Node>>() else {
            logger::log_error("process_weapon_fire_intents: collider не является Node");
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
                logger::log(&format!(
                    "🚫 LOS BLOCKED BY OBSTACLE: shooter {:?} → target {:?} (obstacle: {:?}) - fire intent rejected",
                    intent.shooter, target_entity, collider_id
                ));
                continue;
            };

            // Это actor → проверяем faction
            let Ok(collider_actor) = actors.get(collider_entity) else {
                logger::log(&format!(
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
                logger::log(&format!(
                    "🚫 FRIENDLY FIRE RISK: shooter {:?} (faction {}) won't shoot through ally {:?} (faction {}) at target {:?}",
                    intent.shooter, shooter_actor.faction_id, collider_entity, collider_actor.faction_id, target_entity
                ));
                continue;
            }

            // Враг на линии огня → НЕ стреляем (target switching обработает update_combat_targets_main_thread)
            logger::log(&format!(
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

        logger::log(&format!(
            "Weapon intent APPROVED: shooter {:?} → target {:?} (distance: {:.1}m)",
            intent.shooter, target_entity, distance
        ));
    }
}

/// System: Process WeaponFired events → spawn Godot projectile
/// Создаёт GodotProjectile (полностью Godot-managed, НЕ в ECS)
/// Direction рассчитывается из weapon bone rotation (+Z forward axis)
///
/// ВАЖНО: Fallback direction использует Godot Transform из VisualRegistry!
pub fn weapon_fire_main_thread(
    mut fire_events: EventReader<WeaponFired>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::shared::SceneRoot>,
    mut registry: NonSendMut<crate::projectiles::GodotProjectileRegistry>,
) {
    for event in fire_events.read() {
        // Находим actor node
        let Some(actor_node) = visuals.visuals.get(&event.shooter) else {
            logger::log(&format!("Actor {:?} visual not found", event.shooter));
            continue;
        };

        // 1. Находим BulletSpawn node для spawn_position (Golden Path helper)
        let (spawn_position, weapon_node) = find_bullet_spawn_position(actor_node);

        // 2. Рассчитываем direction из weapon bone rotation
        let direction = if let Some(weapon) = weapon_node {
            // Берём +Z axis weapon bone (наша модель смотрит в +Z, не -Z как Godot convention)
            let global_transform = weapon.get_global_transform();
            let dir = global_transform.basis.col_c();
            logger::log(&format!("🔫 Weapon direction: {:?}", dir));
            dir // basis.z = forward для нашей модели
        } else {
            // Fallback: направление от shooter к target (если есть target)
            if let Some(target_entity) = event.target {
                if let Some(target_node) = visuals.visuals.get(&target_entity) {
                    let shooter_pos = actor_node.get_global_position();
                    let target_pos = target_node.get_global_position();
                    (target_pos - shooter_pos).normalized()
                } else {
                    logger::log("Target visual not found, using default forward");
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

        logger::log(&format!(
            "Spawned projectile: shooter={:?} → target={:?} at {:?} dir={:?} dmg={}",
            event.shooter, event.target, spawn_position, direction, event.damage
        ));
    }
}

// ============================================================================
// Helpers: Bullet Spawn Position + Projectile Creation
// ============================================================================

/// Helper: Find bullet spawn position (BulletSpawn → weapon root → RightHand → actor)
///
/// Returns: (spawn_position, weapon_node_for_direction)
fn find_bullet_spawn_position(actor_node: &Gd<Node3D>) -> (Vector3, Option<Gd<Node3D>>) {
    // Try 1: RightHandAttachment (attachment point)
    let Some(weapon_attachment) = actor_node.try_get_node_as::<Node3D>("%RightHandAttachment") else {
        // Fallback 1: RightHand
        if let Some(right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") {
            logger::log("⚠️ WeaponAttachment not found, using RightHand");
            return (right_hand.get_global_position(), Some(right_hand));
        }

        // Fallback 2: Actor position
        logger::log("⚠️ RightHand not found, using actor position");
        return (actor_node.get_global_position(), None);
    };

    // Try 2: Get weapon prefab (first child of attachment)
    let weapon_prefab = if weapon_attachment.get_child_count() > 0 {
        weapon_attachment.get_child(0).and_then(|node| node.try_cast::<Node3D>().ok())
    } else {
        None
    };

    let Some(weapon_prefab) = weapon_prefab else {
        logger::log("⚠️ No weapon attached to RightHandAttachment");
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
    logger::log("⚠️ BulletSpawn not found (add unique_name_in_owner to weapon prefab)");
    (weapon_prefab.get_global_position(), Some(weapon_prefab))
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

/// Helper: создать GodotProjectile (полностью Godot-managed)
fn spawn_godot_projectile(
    shooter: Entity,
    position: Vector3,
    direction: Vector3,
    speed: f32,
    damage: u32,
    scene_root: &Gd<Node3D>,
    registry: &mut crate::projectiles::GodotProjectileRegistry,
) {
    use crate::projectiles::GodotProjectile;

    // 1. Создаём GodotProjectile node (using IArea3D trait init)
    use godot::classes::IArea3D;
    let mut projectile = Gd::<GodotProjectile>::from_init_fn(|base| {
        <GodotProjectile as IArea3D>::init(base)
    });

    projectile.set_position(position);

    // Collision layers: Projectiles (layer 4)
    // Collision mask: Actors + Environment (projectiles hit actors and walls)
    projectile.set_collision_layer(crate::shared::collision::COLLISION_LAYER_PROJECTILES);
    projectile.set_collision_mask(crate::shared::collision::COLLISION_MASK_PROJECTILES);

    // Debug: проверяем что layers установлены
    logger::log(&format!(
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
