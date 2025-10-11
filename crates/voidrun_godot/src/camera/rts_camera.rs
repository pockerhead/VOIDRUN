use godot::prelude::*;
use godot::classes::{Node3D, Camera3D, InputEvent, InputEventMouseMotion, InputEventMouseButton, Input, input};
use godot::global::{MouseButton, Key};

/// RTS-style camera: WASD movement, mouse drag orbit, scroll zoom
///
/// Hierarchy:
///   RTSCamera3D (root, moves horizontally)
///   └─ RotationX (pitch pivot, -85° to 0°)
///      └─ ZoomPivot (empty node)
///         └─ Camera3D (z-offset for zoom)
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct RTSCamera3D {
    #[base]
    base: Base<Node3D>,

    // Node references (cached in ready)
    rotation_x: Option<Gd<Node3D>>,
    zoom_pivot: Option<Gd<Node3D>>,
    camera: Option<Gd<Camera3D>>,

    // Movement state
    move_target: Vector3,
    move_speed: f32,

    // Rotation state
    rotate_keys_target: f32,  // Y-axis rotation (degrees)
    rotate_keys_speed: f32,
    mouse_sensitivity: f32,

    // Zoom state
    zoom_target: f32,  // Camera Z position
    zoom_speed: f32,
    min_zoom: f32,
    max_zoom: f32,

    // Input state
    is_rotating: bool,  // Is RMB pressed for rotation?
}

#[godot_api]
impl INode3D for RTSCamera3D {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            rotation_x: None,
            zoom_pivot: None,
            camera: None,
            move_target: Vector3::ZERO,
            move_speed: 0.2,
            rotate_keys_target: 0.0,
            rotate_keys_speed: 1.5,
            mouse_sensitivity: 0.2,
            zoom_target: 10.0,  // Default zoom
            zoom_speed: 3.0,
            min_zoom: 0.3,
            max_zoom: 120.0,
            is_rotating: false,
        }
    }

    fn ready(&mut self) {
        // Build node hierarchy if not exists
        self.ensure_hierarchy();

        // Initialize targets from current state
        let pos = self.base().get_position();
        self.move_target = pos;

        let rot = self.base().get_rotation_degrees();
        self.rotate_keys_target = rot.y;

        if let Some(cam) = &self.camera {
            self.zoom_target = cam.get_position().z;
        }

        voidrun_simulation::log("RTSCamera3D ready (Rust)");
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        // Mouse drag rotation (RMB)
        if let Ok(motion) = event.clone().try_cast::<InputEventMouseMotion>() {
            if self.is_rotating {
                let relative = motion.get_relative();

                // Y-axis rotation (horizontal mouse movement)
                self.rotate_keys_target -= relative.x * self.mouse_sensitivity;

                // X-axis pitch (vertical mouse movement) - clamp to [-85, 0]
                if let Some(mut rot_x) = self.rotation_x.clone() {
                    let mut rot = rot_x.get_rotation_degrees();
                    rot.x -= relative.y * self.mouse_sensitivity;
                    rot.x = rot.x.clamp(-85.0, 0.0);
                    rot_x.set_rotation_degrees(rot);
                }
            }
        }

        // Mouse buttons (RMB for rotation, wheel for zoom)
        if let Ok(button) = event.clone().try_cast::<InputEventMouseButton>() {
            match button.get_button_index() {
                MouseButton::RIGHT => {
                    if button.is_pressed() {
                        self.is_rotating = true;
                        Input::singleton().set_mouse_mode(input::MouseMode::CAPTURED);
                    } else {
                        self.is_rotating = false;
                        Input::singleton().set_mouse_mode(input::MouseMode::VISIBLE);
                    }
                }
                MouseButton::WHEEL_UP => {
                    if button.is_pressed() {
                        self.zoom_target -= self.zoom_speed;
                        self.zoom_target = self.zoom_target.clamp(self.min_zoom, self.max_zoom);
                    }
                }
                MouseButton::WHEEL_DOWN => {
                    if button.is_pressed() {
                        self.zoom_target += self.zoom_speed;
                        self.zoom_target = self.zoom_target.clamp(self.min_zoom, self.max_zoom);
                    }
                }
                _ => {}
            }
        }
    }

    fn process(&mut self, _delta: f64) {
        let input = Input::singleton();

        // WASD movement (relative to camera Y rotation)
        let mut input_dir = Vector2::ZERO;

        if input.is_physical_key_pressed(Key::W) {
            input_dir.y -= 1.0;
        }
        if input.is_physical_key_pressed(Key::S) {
            input_dir.y += 1.0;
        }
        if input.is_physical_key_pressed(Key::A) {
            input_dir.x -= 1.0;
        }
        if input.is_physical_key_pressed(Key::D) {
            input_dir.x += 1.0;
        }

        if input_dir.length() > 0.0 {
            input_dir = input_dir.normalized();
        }

        let transform = self.base().get_transform();
        let basis = transform.basis;
        let movement_3d = Vector3::new(input_dir.x, 0.0, input_dir.y);
        let movement_direction = basis * movement_3d;

        self.move_target += movement_direction * self.move_speed;

        // Q/E keyboard rotation
        let mut rotate_keys = 0.0;
        if input.is_physical_key_pressed(Key::Q) {
            rotate_keys -= 1.0;
        }
        if input.is_physical_key_pressed(Key::E) {
            rotate_keys += 1.0;
        }
        self.rotate_keys_target += rotate_keys * self.rotate_keys_speed;

        // Apply smoothing (lerp)
        let current_pos = self.base().get_position();
        let new_pos = current_pos.lerp(self.move_target, 0.05);
        self.base_mut().set_position(new_pos);

        let mut current_rot = self.base().get_rotation_degrees();
        current_rot.y = lerp(current_rot.y, self.rotate_keys_target, 0.05);
        self.base_mut().set_rotation_degrees(current_rot);

        if let Some(mut cam) = self.camera.clone() {
            let mut cam_pos = cam.get_position();
            cam_pos.z = lerp(cam_pos.z, self.zoom_target, 0.10);
            cam.set_position(cam_pos);
        }
    }
}

impl RTSCamera3D {
    /// Build node hierarchy if not exists:
    /// self → RotationX → ZoomPivot → Camera3D
    fn ensure_hierarchy(&mut self) {
        // Find or create RotationX
        let has_rot_x = self.base().has_node("RotationX");
        let node;
        if has_rot_x {
            node = self.base_mut().get_node_as::<Node3D>("RotationX");
        } else {
            let mut rot_x = Node3D::new_alloc();
            rot_x.set_name("RotationX");
            self.base_mut().add_child(&rot_x);
            node = rot_x;
        }
        self.rotation_x = Some(node);

        // Find or create ZoomPivot
        let mut rot_x = self.rotation_x.clone().unwrap();
        let has_zoom_piv = rot_x.has_node("ZoomPivot");
        if has_zoom_piv {
            self.zoom_pivot = Some(rot_x.get_node_as::<Node3D>("ZoomPivot"));
        } else {
            let mut zoom_piv = Node3D::new_alloc();
            zoom_piv.set_name("ZoomPivot");
            rot_x.add_child(&zoom_piv);
            self.zoom_pivot = Some(zoom_piv);
        }

        // Find or create Camera3D
        let mut zoom_piv = self.zoom_pivot.clone().unwrap();
        let has_cam = zoom_piv.has_node("Camera3D");
        if has_cam {
            self.camera = Some(zoom_piv.get_node_as::<Camera3D>("Camera3D"));
        } else {
            let mut cam = Camera3D::new_alloc();
            cam.set_name("Camera3D");
            cam.set_position(Vector3::new(0.0, 0.0, 10.0));  // Initial zoom
            zoom_piv.add_child(&cam);
            self.camera = Some(cam);
        }
    }
}

/// Helper: lerp for f32
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
