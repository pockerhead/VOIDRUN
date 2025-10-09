use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            orbit_camera_controls,
            update_camera_transform,
        ).chain());
    }
}

#[derive(Component)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,   // Horizontal rotation (radians)
    pub pitch: f32, // Vertical rotation (radians)
    pub sensitivity: f32,
    pub zoom_speed: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            distance: 15.0,
            yaw: std::f32::consts::FRAC_PI_4,       // 45°
            pitch: std::f32::consts::FRAC_PI_6,     // 30°
            sensitivity: 0.005,
            zoom_speed: 1.0,
        }
    }
}

/// Handle mouse input for orbit camera
fn orbit_camera_controls(
    mut query: Query<&mut OrbitCamera>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
) {
    let mut camera = match query.single_mut() {
        Ok(cam) => cam,
        Err(_) => return,
    };

    // Right mouse button: orbit
    if mouse_buttons.pressed(MouseButton::Right) {
        for motion in mouse_motion.read() {
            camera.yaw -= motion.delta.x * camera.sensitivity;
            camera.pitch -= motion.delta.y * camera.sensitivity;

            // Clamp pitch to avoid gimbal lock
            camera.pitch = camera.pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.1,
                std::f32::consts::FRAC_PI_2 - 0.1,
            );
        }
    } else {
        // Consume motion events even when not orbiting
        mouse_motion.clear();
    }

    // Mouse wheel: zoom
    for wheel in mouse_wheel.read() {
        camera.distance -= wheel.y * camera.zoom_speed;
        camera.distance = camera.distance.clamp(3.0, 50.0);
    }
}

/// Update camera transform based on orbit parameters
fn update_camera_transform(
    mut query: Query<(&OrbitCamera, &mut Transform), Changed<OrbitCamera>>,
) {
    for (camera, mut transform) in query.iter_mut() {
        // Calculate position from spherical coordinates
        let x = camera.distance * camera.pitch.cos() * camera.yaw.sin();
        let y = camera.distance * camera.pitch.sin();
        let z = camera.distance * camera.pitch.cos() * camera.yaw.cos();

        let position = camera.focus + Vec3::new(x, y, z);

        *transform = Transform::from_translation(position)
            .looking_at(camera.focus, Vec3::Y);
    }
}
