//! Player camera systems (FPS camera setup + toggle + mouse look)
//!
//! # Архитектура
//!
//! **Setup Camera (при spawn player):**
//! - Создаёт Camera3D в Head/CameraPivot
//! - Скрывает Head/Meshes (не видим свою голову в FPS)
//! - Capture mouse
//! - Добавляет ActiveCamera component
//!
//! **Toggle [V] key:**
//! - FPS ↔ RTS camera modes
//! - FPS: player camera active, head meshes hidden, mouse captured
//! - RTS: RTS camera active, head meshes visible, mouse free
//!
//! **Mouse Look (FPS only):**
//! - Horizontal (yaw Y) → rotate Actor body
//! - Vertical (pitch X) → rotate CameraPivot (clamped -30°/+89°)
pub mod rts_camera;

use bevy::prelude::*;
use godot::classes::{Camera3D, Input, input};
use godot::prelude::*;
use voidrun_simulation::camera::{ActiveCamera, CameraMode};
use voidrun_simulation::player::Player;
use voidrun_simulation::PrefabPath;
use voidrun_simulation::logger;

use crate::input::{CameraToggleEvent, MouseLookEvent};
use crate::shared::{SceneRoot, VisualRegistry};

/// Setup player camera при spawn
///
/// # Действия
/// - Find Head/CameraPivot node
/// - Create Camera3D as child
/// - Set active camera
/// - Hide Head/Meshes (FPS mode)
/// - Capture mouse
/// - Add ActiveCamera component
///
/// # Schedule
/// - PostUpdate (после attach_prefabs_main_thread)
pub fn setup_player_camera(
    player_query: Query<Entity, (With<Player>, Added<PrefabPath>)>,
    visuals: NonSend<VisualRegistry>,
    mut commands: Commands,
) {
    for player_entity in player_query.iter() {
        let Some(player_node) = visuals.visuals.get(&player_entity) else {
            continue;
        };

        // Find CameraPivot (unique name)
        let Some(mut camera_pivot) = player_node.try_get_node_as::<godot::classes::Node3D>("%CameraPivot") else {
            logger::log_error("❌ CameraPivot not found in test_player.tscn! Check scene structure.");
            continue;
        };

        // Create Camera3D as child of CameraPivot
        let mut camera = Camera3D::new_alloc();
        camera.set_name("PlayerCamera");
        camera.set_fov(90.0);
        camera.set_current(true); // Make active

        camera_pivot.add_child(&camera.upcast::<godot::classes::Node>());

        // Hide head meshes (первый person не видит свою голову)
        if let Some(mut head_meshes) = player_node.try_get_node_as::<godot::classes::Node3D>("%HeadMeshes") {
            head_meshes.set_visible(false);
        }

        // Add ActiveCamera component (track mode)
        commands.entity(player_entity).insert(ActiveCamera {
            mode: CameraMode::FirstPerson,
        });

        // Capture mouse
        Input::singleton().set_mouse_mode(input::MouseMode::CAPTURED);

        logger::log("✅ Player FPS camera ready (V to toggle, mouse to look)");
    }
}

/// Camera toggle system - [V] key переключает FPS ↔ RTS
///
/// # Эффекты
/// - FPS mode: player camera active, head meshes hidden, mouse captured
/// - RTS mode: RTS camera active, head meshes visible, mouse free
///
/// # Schedule
/// - Update (обрабатываем input events)
pub fn camera_toggle_system(
    mut events: EventReader<CameraToggleEvent>,
    mut player_query: Query<(&mut ActiveCamera, Entity), With<Player>>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<SceneRoot>,
) {
    let Ok((mut active_camera, player_entity)) = player_query.get_single_mut() else {
        return;
    };

    for _event in events.read() {
        // Toggle mode
        let new_mode = match active_camera.mode {
            CameraMode::FirstPerson => CameraMode::RTS,
            CameraMode::RTS => CameraMode::FirstPerson,
        };

        active_camera.mode = new_mode;

        match new_mode {
            CameraMode::FirstPerson => {
                // Activate player camera
                let Some(player_node) = visuals.visuals.get(&player_entity) else {
                    continue;
                };
                let Some(mut player_camera) = player_node.try_get_node_as::<Camera3D>("%CameraPivot/PlayerCamera") else {
                    logger::log_error("❌ PlayerCamera not found!");
                    continue;
                };

                player_camera.set_current(true);

                // Hide head meshes
                if let Some(mut head_meshes) = player_node.try_get_node_as::<godot::classes::Node3D>("%HeadMeshes") {
                    head_meshes.set_visible(false);
                }

                // Capture mouse
                Input::singleton().set_mouse_mode(input::MouseMode::CAPTURED);

                logger::log("📷 First-Person Camera");
            }

            CameraMode::RTS => {
                // Activate RTS camera (find through scene root)
                // RTSCamera3D/RotationX/ZoomPivot/Camera3D
                let Some(mut rts_camera) = scene_root
                    .node
                    .try_get_node_as::<Camera3D>("RTSCamera3D/RotationX/ZoomPivot/Camera3D")
                else {
                    logger::log_error("❌ RTS Camera not found in scene!");
                    continue;
                };

                rts_camera.set_current(true);

                // Show head meshes (RTS view видит голову)
                let Some(player_node) = visuals.visuals.get(&player_entity) else {
                    continue;
                };
                if let Some(mut head_meshes) = player_node.try_get_node_as::<godot::classes::Node3D>("%HeadMeshes") {
                    head_meshes.set_visible(true);
                }

                // Release mouse
                Input::singleton().set_mouse_mode(input::MouseMode::VISIBLE);

                logger::log("📷 RTS Camera (strategic view)");
            }
        }
    }
}

/// Player mouse look system - rotate camera по mouse motion (FPS only)
///
/// # Rotation
/// - Horizontal (yaw Y) → rotate Actor body
/// - Vertical (pitch X) → rotate CameraPivot (clamped -30°/+89°)
///
/// # Pitch Limits
/// - Up: +89° (почти вертикаль вверх, не ровно 90° для stability)
/// - Down: -30° (до груди, не слишком низко)
///
/// # Schedule
/// - Update (обрабатываем mouse motion events)
pub fn player_mouse_look(
    mut mouse_events: EventReader<MouseLookEvent>,
    player_query: Query<(Entity, &ActiveCamera), With<Player>>,
    visuals: NonSend<VisualRegistry>,
) {
    let Ok((player_entity, active_camera)) = player_query.get_single() else {
        return;
    };

    // Only в FPS mode
    if active_camera.mode != CameraMode::FirstPerson {
        return;
    }

    let Some(player_node) = visuals.visuals.get(&player_entity) else {
        return;
    };

    for event in mouse_events.read() {
        const MOUSE_SENSITIVITY: f32 = 0.002; // Радианы за pixel (стандарт FPS)

        // Yaw (Y axis) - rotate player body
        let mut player_node_mut = player_node.clone();
        let mut player_rot = player_node_mut.get_rotation();
        player_rot.y -= event.delta_x * MOUSE_SENSITIVITY;
        player_node_mut.set_rotation(player_rot);

        // Pitch (X axis) - rotate CameraPivot (clamped)
        let Some(mut camera_pivot) = player_node_mut.try_get_node_as::<godot::classes::Node3D>("%CameraPivot")
        else {
            continue;
        };

        let mut camera_rot = camera_pivot.get_rotation();
        camera_rot.x -= event.delta_y * MOUSE_SENSITIVITY;

        // Clamp pitch: -30° (down to chest) / +89° (up almost vertical)
        const PITCH_DOWN_LIMIT: f32 = -80.0_f32.to_radians();
        const PITCH_UP_LIMIT: f32 = 89.0_f32.to_radians();
        camera_rot.x = camera_rot.x.clamp(PITCH_DOWN_LIMIT, PITCH_UP_LIMIT);

        camera_pivot.set_rotation(camera_rot);
    }
}
