//! Player Shooting Systems - ADS and Hip Fire mechanics
//!
//! Architecture:
//! - Procedural weapon positioning (NOT keyframe animations)
//! - Sight Socket Method (Unreal Engine technique)
//! - Manual lerp for smooth transitions
//!
//! Systems:
//! 1. process_ads_toggle - Handle RMB toggle intent
//! 2. update_ads_position_transition - Smooth lerp Hip↔ADS
//! 3. player_hip_fire_aim - Dynamic raycast targeting
//!
//! Flow:
//! RMB → ToggleADSIntent → process_ads_toggle → update transition state
//!                                             ↓
//!                          update_ads_position_transition (lerp position)
//!                                             ↓
//!                          player_hip_fire_aim (if Hip Fire mode)

use bevy::prelude::*;
use godot::prelude::*;
use godot::classes::Node3D;
use godot::builtin::Transform3D as GodotTransform3D;

use voidrun_simulation::player::Player;
use voidrun_simulation::shooting::{AimMode, ToggleADSIntent, ease_out_cubic};
use voidrun_simulation::logger;
use crate::shared::{VisualRegistry, SceneRoot, AttachmentRegistry, GodotDeltaTime};

// ============================================================================
// Helper Functions
// ============================================================================

/// Get active Camera3D from viewport
///
/// Returns global transform of active camera or None
fn get_active_camera(scene_root: &SceneRoot) -> Option<GodotTransform3D> {
    let viewport = scene_root.node.get_viewport()?;
    let camera = viewport.get_camera_3d()?;
    Some(camera.get_global_transform())
}

/// Calculate target transform for RightHand in ADS mode (CameraLine method)
///
/// Simplified approach:
/// 1. Find CameraLine node (unique in player prefab)
/// 2. Find SightSocket in weapon (unique in weapon prefab)
/// 3. RightHand transform = align SightSocket with CameraLine
///
/// # Returns
///
/// Target transform for RightHand (world space) - position + rotation
pub fn calculate_ads_target_transform_cameraline(
    player_actor_node: &Gd<Node3D>,
    weapon_node: &Gd<Node3D>,
) -> Option<(Vector3, Vector3)> {
    // 1. Найти CameraPivot (parent CameraLine) для rotation
    let Some(camera_pivot_node) = player_actor_node.get_node_or_null("%CameraPivot") else {
        logger::log_error("❌ CameraPivot не найден (нужен unique name в player prefab)");
        return None;
    };

    let Ok(camera_pivot) = camera_pivot_node.try_cast::<Node3D>() else {
        return None;
    };

    // 2. CameraLine для position
    let Some(camera_line) = player_actor_node.get_node_or_null("%CameraLine") else {
        logger::log_error("❌ CameraLine не найден (нужен unique name в player prefab)");
        return None;
    };

    let Ok(camera_line_3d) = camera_line.try_cast::<Node3D>() else {
        return None;
    };

    // 2. Найти SightSocket в weapon через unique name
    let Some(sight_socket_node) = weapon_node.get_node_or_null("%SightSocket") else {
        logger::log_error("❌ SightSocket не найден в weapon (нужен unique name)");
        return None;
    };

    let Ok(sight_socket) = sight_socket_node.try_cast::<Node3D>() else {
        return None;
    };

    // 3. Get camera transform (для position + rotation)
    let camera_pivot_transform = camera_pivot.get_global_transform();
    let camera_backward = camera_pivot_transform.basis.col_c(); // +Z = назад к камере
    let camera_forward = -camera_pivot_transform.basis.col_c(); // -Z = вперёд от камеры

    // 4. Вычисляем target position для RightHand
    let camera_line_global = camera_line_3d.get_global_position();
    let sight_socket_global = sight_socket.get_global_position();
    let weapon_root_global = weapon_node.get_global_position();

    // Offset от weapon root до SightSocket (world space)
    let sight_offset = sight_socket_global - weapon_root_global;

    // Target RightHand position
    // Добавляем small offset назад к игроку (ближе к камере)
    const ADS_OFFSET_TOWARDS_CAMERA: f32 = 0.40; // 15cm ближе к игроку (TUNEABLE!)
    let target_hand_position = camera_line_global - sight_offset + camera_backward * ADS_OFFSET_TOWARDS_CAMERA;

    // 5. Target rotation
    let target_look_at = target_hand_position + camera_forward * 10.0;

    Some((target_hand_position, target_look_at))
}

/// Physics raycast helper (camera → world)
///
/// # Returns
///
/// Hit position or None if no collision
pub fn camera_raycast_hit_point(
    scene_root: &SceneRoot,
    camera_pos: Vector3,
    camera_forward: Vector3,
    max_distance: f32,
) -> Option<Vector3> {
    let mut world = scene_root.node.get_world_3d()?;
    let mut space = world.get_direct_space_state()?;

    let target_pos = camera_pos + camera_forward * max_distance;

    let query_params = godot::classes::PhysicsRayQueryParameters3D::create(camera_pos, target_pos)?;
    let mut query = query_params;

    query.set_collision_mask(crate::collision_layers::COLLISION_MASK_RAYCAST_LOS);

    let result = space.intersect_ray(&query);

    if result.is_empty() {
        return None;
    }

    let position_variant = result.get("position")?;
    let position = position_variant.try_to::<Vector3>().ok()?;

    Some(position)
}

// ============================================================================
// System 1: Process ADS Toggle (RMB Input)
// ============================================================================

/// System: Process ToggleADSIntent events (RMB pressed)
///
/// Flow:
/// - HipFire → start EnteringADS transition
/// - ADS → start ExitingADS transition
/// - Transitioning → ignore (prevent spam)
///
/// Captures current RightHand position as transition start_position
pub fn process_ads_toggle(
    mut toggle_events: EventReader<ToggleADSIntent>,
    mut player_query: Query<&mut AimMode, With<Player>>,
    visuals: NonSend<VisualRegistry>,
) {
    for intent in toggle_events.read() {
        let Ok(mut aim_mode) = player_query.get_mut(intent.entity) else {
            continue;
        };

        // Get current RightHand position (start of transition)
        let Some(actor_node) = visuals.visuals.get(&intent.entity) else {
            continue;
        };

        let Some(right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") else {
            continue;
        };

        let current_position = right_hand.get_global_position();

        match aim_mode.as_ref() {
            AimMode::HipFire => {
                // Start entering ADS
                *aim_mode = AimMode::EnteringADS {
                    start_position: Vec3::new(current_position.x, current_position.y, current_position.z),
                    progress: 0.0,
                };

                logger::log("▶️ Entering ADS");
            }

            AimMode::ADS => {
                // Start exiting ADS
                *aim_mode = AimMode::ExitingADS {
                    start_position: Vec3::new(current_position.x, current_position.y, current_position.z),
                    progress: 0.0,
                };

                logger::log("◀️ Exiting ADS");
            }

            _ => {
                // Already transitioning, ignore (prevent spam)
            }
        }
    }
}

// ============================================================================
// System 2: Update ADS Position Transition (Smooth Lerp)
// ============================================================================

/// System: Update ADS position transitions + continuous ADS positioning
///
/// Runs EVERY frame for:
/// - EnteringADS: lerp from hip → ads (0.3s)
/// - ADS: continuously update position (camera can rotate!)
/// - ExitingADS: lerp from ads → hip (0.3s)
///
/// **CRITICAL:** Must run AFTER Godot animations but BEFORE other aim systems!
pub fn update_ads_position_transition(
    mut player_query: Query<(&mut AimMode, Entity), With<Player>>,
    visuals: NonSend<VisualRegistry>,
    attachments: NonSend<AttachmentRegistry>,
    scene_root: NonSend<SceneRoot>,
    time: Res<GodotDeltaTime>,
) {
    for (mut aim_mode, entity) in player_query.iter_mut() {
        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let Some(mut right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") else {
            continue;
        };

        match aim_mode.as_mut() {
            AimMode::EnteringADS { start_position, progress } => {
                // Update progress
                *progress += time.0 / AimMode::TRANSITION_DURATION;

                if *progress >= 1.0 {
                    // Transition complete
                    *aim_mode = AimMode::ADS;
                } else {
                    // Calculate target transform (CameraLine method)
                    let Some(actor_node) = visuals.visuals.get(&entity) else {
                        continue;
                    };

                    let weapon_key = (entity, "%RightHandAttachment".to_string());
                    let Some(weapon_node) = attachments.attachments.get(&weapon_key) else {
                        continue;
                    };

                    let Some((target_pos, target_look_at)) = calculate_ads_target_transform_cameraline(
                        actor_node,
                        weapon_node,
                    ) else {
                        continue;
                    };

                    // Smooth lerp with ease-out curve
                    let t = ease_out_cubic(*progress);
                    let start_vec = Vector3::new(start_position.x, start_position.y, start_position.z);
                    let current_pos = start_vec.lerp(target_pos, t);

                    right_hand.set_global_position(current_pos);
                    right_hand.look_at(target_look_at); // Rotate to match camera direction
                }
            }

            AimMode::ExitingADS { start_position, progress } => {
                // Similar logic but reverse (ADS → Hip Fire)
                *progress += time.0 / AimMode::TRANSITION_DURATION;

                if *progress >= 1.0 {
                    *aim_mode = AimMode::HipFire;
                    // Reset to local position (animation will control)
                    right_hand.set_position(Vector3::new(-0.5, 0.0, 0.0));
                } else {
                    // Lerp from current ADS position to hip fire base position
                    let hip_fire_pos_local = Vector3::new(-0.5, 0.0, 0.0);

                    // Get actor node to convert local to global
                    let Some(actor_node) = visuals.visuals.get(&entity) else {
                        continue;
                    };
                    let actor_transform = actor_node.get_global_transform();
                    let hip_fire_pos_global = actor_transform * hip_fire_pos_local;

                    let t = ease_out_cubic(*progress);
                    let start_vec = Vector3::new(start_position.x, start_position.y, start_position.z);
                    let current_pos = start_vec.lerp(hip_fire_pos_global, t);

                    right_hand.set_global_position(current_pos);
                }
            }

            AimMode::ADS => {
                // Continuously update position + rotation (camera может двигаться!)
                let Some(actor_node) = visuals.visuals.get(&entity) else {
                    continue;
                };

                let weapon_key = (entity, "%RightHandAttachment".to_string());
                let Some(weapon_node) = attachments.attachments.get(&weapon_key) else {
                    continue;
                };

                let Some((target_pos, target_look_at)) = calculate_ads_target_transform_cameraline(
                    actor_node,
                    weapon_node,
                ) else {
                    continue;
                };

                right_hand.set_global_position(target_pos);
                right_hand.look_at(target_look_at); // Match camera pitch/yaw
            }

            AimMode::HipFire => {
                // Handled by player_hip_fire_aim
            }
        }
    }
}

// ============================================================================
// System 3: Player Hip Fire Aim (Dynamic Raycast)
// ============================================================================

/// System: Aim weapon in Hip Fire mode (dynamic raycast targeting)
///
/// Flow:
/// 1. Camera raycast (50m max)
/// 2. If hit → aim to hit.position
/// 3. If no hit → aim to camera_pos + forward * 50m
/// 4. RightHand.look_at(aim_target)
///
/// **Only runs in Hip Fire mode!**
pub fn player_hip_fire_aim(
    player_query: Query<(Entity, &AimMode), With<Player>>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<SceneRoot>,
) {
    for (entity, aim_mode) in player_query.iter() {
        // Только Hip Fire mode
        if !matches!(aim_mode, AimMode::HipFire) {
            continue;
        }

        let Some(actor_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        let Some(mut right_hand) = actor_node.try_get_node_as::<Node3D>("RightHand") else {
            continue;
        };

        // Get active camera
        let Some(camera_transform) = get_active_camera(&scene_root) else {
            continue;
        };

        // Camera raycast (50m max)
        let camera_pos = camera_transform.origin;
        let camera_forward = -camera_transform.basis.col_c(); // -Z = forward in Godot

        let raycast_result = camera_raycast_hit_point(
            &scene_root,
            camera_pos,
            camera_forward,
            50.0,  // max distance
        );

        // Aim target: hit point или fallback 50m
        let aim_target = raycast_result.unwrap_or(camera_pos + camera_forward * 50.0);

        // RightHand look_at aim target
        right_hand.look_at(aim_target);

        logger::log(&format!(
            "🎯 Hip Fire aim: camera_forward={:?}, aim_target={:?}",
            camera_forward, aim_target
        ));
    }
}
