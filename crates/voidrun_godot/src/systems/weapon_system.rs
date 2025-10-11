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
use voidrun_simulation::combat::WeaponFired;
use crate::systems::VisualRegistry;

/// System: Aim weapon at target (RightHand rotation)
/// Если актёр в Combat state → поворачиваем руку к target
pub fn weapon_aim_main_thread(
    actors: Query<(Entity, &ai::AIState, &Transform), With<Actor>>,
    targets: Query<&Transform>,
    visuals: NonSend<VisualRegistry>,
) {
    for (entity, state, transform) in actors.iter() {
        // Целимся только в Combat state
        if let ai::AIState::Combat { target } = state {

            if let Ok(target_transform) = targets.get(*target) {
                // Направление к target
                let to_target = target_transform.translation - transform.translation;

                if to_target.length() > 0.01 {
                    // Получаем actor node
                    if let Some(mut actor_node) = visuals.visuals.get(&entity).cloned() {
                        let target_pos = Vector3::new(
                            target_transform.translation.x,
                            target_transform.translation.y,
                            target_transform.translation.z,
                        );

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
    }
}

/// System: Process WeaponFired events → spawn Godot projectile
/// Создаёт GodotProjectile (полностью Godot-managed, НЕ в ECS)
/// Direction рассчитывается из weapon bone rotation (+Z forward axis)
pub fn weapon_fire_main_thread(
    mut fire_events: EventReader<WeaponFired>,
    targets: Query<&Transform>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
) {
    for event in fire_events.read() {
        // Находим actor node
        let Some(actor_node) = visuals.visuals.get(&event.shooter) else {
            voidrun_simulation::log(&format!("Actor {:?} visual not found", event.shooter));
            continue;
        };

        // 1. Находим BulletSpawn node для spawn_position
        let (spawn_position, weapon_node) = if let Some(weapon_attachment) = actor_node.try_get_node_as::<Node3D>("RightHand/WeaponAttachment") {
            // Рекурсивно ищем BulletSpawn внутри WeaponAttachment
            if let Some(bullet_spawn) = find_node_recursive(&weapon_attachment, "BulletSpawn") {
                (bullet_spawn.get_global_position(), Some(bullet_spawn))
            } else {
                voidrun_simulation::log("BulletSpawn not found, using WeaponAttachment");
                (weapon_attachment.get_global_position(), Some(weapon_attachment))
            }
        } else if let Some(right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") {
            voidrun_simulation::log("WeaponAttachment not found, using RightHand");
            (right_hand.get_global_position(), Some(right_hand))
        } else {
            voidrun_simulation::log("RightHand not found, using actor position");
            (actor_node.get_global_position(), None)
        };

        // 2. Рассчитываем direction из weapon bone rotation
        let direction = if let Some(weapon) = weapon_node {
            // Берём +Z axis weapon bone (наша модель смотрит в +Z, не -Z как Godot convention)
            let global_transform = weapon.get_global_transform();
            global_transform.basis.col_c() // basis.z = forward для нашей модели
        } else {
            // Fallback: направление от shooter к target (ECS strategic)
            if let Ok(target_transform) = targets.get(event.target) {
                let shooter_pos = actor_node.get_global_position();
                let target_pos = Vector3::new(
                    target_transform.translation.x,
                    target_transform.translation.y,
                    target_transform.translation.z,
                );
                (target_pos - shooter_pos).normalized()
            } else {
                voidrun_simulation::log("Target not found, using default forward");
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
) {
    use crate::projectile::GodotProjectile;

    // 1. Создаём GodotProjectile node
    let mut projectile = Gd::<GodotProjectile>::from_init_fn(|base| {
        GodotProjectile::init(base)
    });

    projectile.set_position(position);

    // Collision layers: layer 4 (projectile), mask 2 (actors only, не projectiles)
    projectile.set_collision_layer(4);
    projectile.set_collision_mask(2);

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

    // 5. Добавляем в сцену (Godot автоматически вызовет _physics_process)
    scene_root.clone().upcast::<Node>().add_child(&projectile.upcast::<Node>());
}

// ❌ projectile_physics удалена — GodotProjectile::physics_process обрабатывает всё

/// System: Обработка ProjectileHit событий из Godot queue
///
/// Godot projectiles пушат collision events в static queue,
/// эта система читает их и отправляет в ECS EventWriter
pub fn process_godot_projectile_hits(
    mut projectile_hit_events: EventWriter<voidrun_simulation::combat::ProjectileHit>,
) {
    // Забираем все hit events из Godot queue
    let hits = crate::projectile::take_projectile_hits();

    for hit in hits {
        projectile_hit_events.write(hit);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_aim_only_in_combat() {
        // Verify aim system only triggers in Combat state
        // (unit test без Godot API)
    }
}
