//! Player input systems (ECS)
//!
//! Обрабатывают PlayerInputEvent и НАПРЯМУЮ управляют CharacterBody3D velocity.
//!
//! # Архитектура
//!
//! **Player НЕ использует NavigationAgent/MovementCommand!**
//! - AI actors: Input → MovementCommand → NavigationAgent → velocity
//! - Player: Input → НАПРЯМУЮ CharacterBody3D velocity (FPS-style)
//!
//! **Почему:**
//! - Player не нуждается в pathfinding (мы управляем direction напрямую)
//! - Нет lag от NavigationAgent processing
//! - Прямой контроль = responsive gameplay

use bevy::prelude::*;
use godot::prelude::*;
use voidrun_simulation::components::{ActiveCamera, CameraMode, JumpIntent, Player};
use voidrun_simulation::combat::{MeleeAttackIntent, MeleeAttackState, ParryIntent, ParryState, WeaponStats};

use super::events::PlayerInputEvent;
use crate::systems::VisualRegistry;

/// Player movement system - НАПРЯМУЮ устанавливает velocity CharacterBody3D
///
/// # Архитектура
/// - Читает: PlayerInputEvent (from PlayerInputController)
/// - Пишет: CharacterBody3D.velocity (НАПРЯМУЮ через Godot API)
/// - Query: With<Player> (только player-controlled actors)
///
/// # Movement
/// - WASD → CharacterBody3D.velocity (FPS-style direct control)
/// - Sprint → speed multiplier (6.0 vs 3.0 м/с)
/// - Space → JumpIntent event (обрабатывается gravity system)
///
/// # Camera-Relative Movement (FPS mode)
/// - FPS mode: WASD относительно Actor body rotation (yaw Y)
/// - RTS mode: WASD relative to world axes (legacy behavior)
/// - W/S → forward/back (Actor forward, projected на XZ plane)
/// - A/D → strafe left/right (perpendicular to Actor forward)
///
/// # Важно
/// - НЕ используем MovementCommand (это для AI pathfinding)
/// - НЕ используем NavigationAgent (это для AI avoidance)
/// - Прямое управление velocity как в FPS играх
pub fn process_player_input(
    mut input_events: EventReader<PlayerInputEvent>,
    mut jump_events: EventWriter<JumpIntent>,
    player_query: Query<(Entity, Option<&ActiveCamera>), With<Player>>,
    visuals: NonSend<VisualRegistry>,
) {
    // Guard: нет player entity
    let Ok((player_entity, active_camera)) = player_query.get_single() else {
        return;
    };

    // Get Godot CharacterBody3D node
    let Some(player_node_3d) = visuals.visuals.get(&player_entity) else {
        return;
    };

    let Ok(mut player_body) = player_node_3d
        .clone()
        .try_cast::<godot::classes::CharacterBody3D>()
    else {
        return;
    };

    // Check if FPS mode
    let is_fps = active_camera
        .map(|c| c.mode == CameraMode::FirstPerson)
        .unwrap_or(false);

    for input in input_events.read() {
        // WASD movement - НАПРЯМУЮ velocity
        if !input.move_direction.is_nan() && input.move_direction.length_squared() > 0.01 {
            let speed = if input.sprint { 6.0 } else { 3.0 }; // unlimited sprint

            let velocity = if is_fps {
                // FPS mode: camera-relative movement (Actor body rotation)
                // Паттерн из 3d-rpg player.gd:
                // var input_vector := Vector3(input_dir.x, 0, input_dir.y).normalized()
                // var direction := horizontal_pivot.global_transform.basis * input_vector

                // 1. Создаём input vector в локальном пространстве (x, 0, z) и normalize
                let input_vector = godot::prelude::Vector3::new(
                    input.move_direction.x,
                    0.0,
                    input.move_direction.y,
                ).normalized();

                // 2. Получаем basis из Actor transform (yaw rotation)
                let actor_transform = player_node_3d.get_global_transform();
                let actor_basis = actor_transform.basis;

                // 3. Преобразуем локальный input в world space через basis multiplication
                // direction := horizontal_pivot.global_transform.basis * input_vector
                let direction = actor_basis * input_vector;

                godot::prelude::Vector3::new(
                    direction.x * speed,
                    player_body.get_velocity().y, // Keep Y (gravity)
                    direction.z * speed,
                )
            } else {
                // RTS mode: world-space movement (legacy)
                godot::prelude::Vector3::new(
                    input.move_direction.x * speed,
                    player_body.get_velocity().y,
                    input.move_direction.y * speed,
                )
            };

            player_body.set_velocity(velocity);
        } else {
            // No movement input → stop horizontal movement (keep Y for gravity)
            let mut velocity = player_body.get_velocity();
            velocity.x = 0.0;
            velocity.z = 0.0;
            player_body.set_velocity(velocity);
        }

        // Jump
        if input.jump {
            jump_events.write(JumpIntent {
                entity: player_entity,
            });
        }
    }
    player_body.move_and_slide();
}

/// Player combat input system - обрабатывает attack/parry input
///
/// # Архитектура
/// - Читает: PlayerInputEvent
/// - Пишет: MeleeAttackIntent (attack), ParryIntent (parry)
/// - Query: With<Player>
///
/// # Combat
/// - LMB → MeleeAttackIntent (area-based collision detection)
/// - RMB → ParryIntent (VisionCone-based target detection + mutual facing check)
///
/// # Parry Detection
/// - Uses player VisionCone to find visible enemies
/// - Checks `actors_facing_each_other()` (mutual facing)
/// - Requires attacker in Windup phase
/// - Maximum distance: 3m
pub fn player_combat_input(
    mut input_events: EventReader<PlayerInputEvent>,
    mut attack_events: EventWriter<MeleeAttackIntent>,
    mut parry_events: EventWriter<ParryIntent>,
    player_query: Query<Entity, With<Player>>,
    attack_states: Query<(Entity, &MeleeAttackState)>,
    parry_states: Query<&ParryState>,
    weapons: Query<&WeaponStats>,
    visuals: NonSend<VisualRegistry>,
) {
    // Guard: нет player entity
    let Ok(player_entity) = player_query.single() else {
        return;
    };

    for input in input_events.read() {
        // LMB → Attack (melee or ranged depending on weapon type)
        if input.attack {
            // Check weapon type
            let Ok(weapon_stats) = weapons.get(player_entity) else {
                continue;
            };

            if weapon_stats.is_melee() {
                // Melee attack (area-based, no target needed)
                attack_events.write(MeleeAttackIntent {
                    attacker: player_entity,
                    attack_type: voidrun_simulation::combat::MeleeAttackType::Normal,
                });
            } else if weapon_stats.is_ranged() {
                // TODO: Ranged attack (emit RangedAttackIntent)
                voidrun_simulation::log("🔫 Ranged attack not implemented yet (Phase 5)");
            }
        }

        // RMB → Parry (always allowed - targeted or idle)
        if input.parry {
            // Guard 1: Already parrying
            if parry_states.contains(player_entity) {
                voidrun_simulation::log("⚠️ Player already parrying");
                continue;
            }

            // Guard 2: Attacking (cannot parry during attack)
            if attack_states.iter().any(|(e, _)| e == player_entity) {
                voidrun_simulation::log("⚠️ Cannot parry while attacking");
                continue;
            }

            // Find closest attacker in vision (optional)
            let attacker = find_closest_attacker_in_vision(
                player_entity,
                &attack_states,
                &weapons,
                &visuals,
            )
            .map(|(entity, _windup)| entity); // Take only Entity, ignore windup

            // ALWAYS generate ParryIntent (даже если нет attacker)
            parry_events.write(ParryIntent {
                defender: player_entity,
                attacker, // Some(entity) or None
                expected_windup_duration: 0.0, // Unused
            });

            // Log based on parry type
            if let Some(target) = attacker {
                voidrun_simulation::log(&format!("🛡️ Player parry → target {:?}", target));
            } else {
                voidrun_simulation::log("🛡️ Player parry (defensive/idle)");
            }
        }
    }
}

// ============================================================================
// Helper Functions: Parry Target Detection
// ============================================================================

/// Find closest attacking enemy in player's vision cone.
///
/// Uses VisionCone collision detection + mutual facing check.
///
/// # Requirements
/// - Enemy has `MeleeAttackState` (Windup phase only)
/// - Enemy is visible (in VisionCone overlaps)
/// - Mutual facing check (`actors_facing_each_other`)
/// - Distance ≤ MAX_PARRY_DISTANCE (3m)
///
/// # Returns
/// - `Some((attacker_entity, expected_windup_duration))` if found
/// - `None` if no valid targets
fn find_closest_attacker_in_vision(
    player: Entity,
    attack_states: &Query<(Entity, &MeleeAttackState)>,
    weapons: &Query<&WeaponStats>,
    visuals: &NonSend<VisualRegistry>,
) -> Option<(Entity, f32)> {
    const MAX_PARRY_DISTANCE: f32 = 3.0;

    // Get player node
    let player_node = visuals.visuals.get(&player)?;

    // Get player VisionCone Area3D (path: Head/VisionCone)
    let Some(vision_cone) = player_node.try_get_node_as::<godot::classes::Area3D>("Head/VisionCone")
    else {
        voidrun_simulation::log_error("❌ Player VisionCone not found (parry detection failed)");
        return None;
    };

    // Get all overlapping bodies (visible actors)
    let overlaps = vision_cone.get_overlapping_bodies();

    let mut closest: Option<(Entity, f32, f32)> = None; // (entity, distance, expected_windup)

    for i in 0..overlaps.len() {
        let Some(body) = overlaps.get(i) else {
            continue;
        };

        // Find entity for this Godot node
        let Some(enemy_entity) = find_entity_for_node(&body.upcast::<godot::classes::Node>(), visuals) else {
            continue;
        };

        // Skip self
        if enemy_entity == player {
            continue;
        }

        // Check if attacking (Windup phase only)
        let Some((_, attack_state)) = attack_states.iter().find(|(e, _)| *e == enemy_entity)
        else {
            continue;
        };

        if !attack_state.is_windup() {
            continue;
        }

        // Get enemy node for facing/distance check
        let Some(enemy_node) = visuals.visuals.get(&enemy_entity) else {
            continue;
        };

        // Mutual facing check (both actors looking at each other)
        use crate::actor_utils::{actors_facing_each_other, angles};
        if actors_facing_each_other(player_node, enemy_node, angles::MODERATE_45_DEG).is_none() {
            continue; // Not mutually facing
        }

        // Distance check
        let distance = player_node
            .get_global_position()
            .distance_to(enemy_node.get_global_position());

        if distance > MAX_PARRY_DISTANCE {
            continue;
        }

        // Get expected windup duration
        let Ok(weapon) = weapons.get(enemy_entity) else {
            continue;
        };
        let expected_windup = weapon.windup_duration;

        // Track closest
        if closest.is_none() || distance < closest.unwrap().1 {
            closest = Some((enemy_entity, distance, expected_windup));
        }
    }

    closest.map(|(entity, _, windup)| (entity, windup))
}

/// Find ECS entity for Godot Node3D (reverse lookup).
///
/// Uses VisualRegistry::node_to_entity HashMap for O(1) lookup.
fn find_entity_for_node(node: &Gd<godot::classes::Node>, visuals: &VisualRegistry) -> Option<Entity> {
    let node_id = node.instance_id();
    visuals.node_to_entity.get(&node_id).copied()
}
