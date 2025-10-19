//! Player input events
//!
//! События генерируются из Godot Input API (PlayerInputController)
//! и обрабатываются ECS systems.

use bevy::prelude::Event;
use bevy::math::Vec2;

/// Player input event - генерируется каждый frame когда есть player input
///
/// # Архитектура
/// - Emit: PlayerInputController (Godot node) в `process()`
/// - Consume: player_movement_system, player_combat_input (ECS systems)
///
/// # Fields
/// - `move_direction`: WASD input (normalized, Vec2::ZERO если нет движения)
/// - `sprint`: Shift key (unlimited sprint, stamina не тратится пока)
/// - `jump`: Space key (just_pressed)
/// - `attack`: LMB (just_pressed)
/// - `parry`: RMB (just_pressed)
///
/// # Примечание
/// Mouse look пока НЕ включён (камера будет позже)
#[derive(Event, Debug, Clone, Copy, Default)]
pub struct PlayerInputEvent {
    /// WASD movement direction (normalized)
    ///
    /// # Coordinate System
    /// Logical direction independent of Godot conventions:
    /// - `x`: -1.0 (left) → +1.0 (right)
    /// - `y`: -1.0 (forward, maps to Godot -Z) → +1.0 (backward, maps to Godot +Z)
    ///
    /// # Examples
    /// - W key: `Vec2(0, -1)` → forward (Godot -Z)
    /// - S key: `Vec2(0, 1)` → backward (Godot +Z)
    /// - A key: `Vec2(-1, 0)` → left (Godot -X)
    /// - D key: `Vec2(1, 0)` → right (Godot +X)
    /// - W+D diagonal: `Vec2(0.707, -0.707)` (normalized)
    pub move_direction: Vec2,

    /// Sprint key (Shift) - пока unlimited (stamina не тратится)
    pub sprint: bool,

    /// Jump key (Space) - just_pressed
    pub jump: bool,

    /// Primary action (LMB) - just_pressed
    /// - Melee weapon: attack
    /// - Ranged weapon: fire
    pub primary_action: bool,

    /// Secondary action (RMB) - just_pressed
    /// - Melee weapon: parry
    /// - Ranged weapon: toggle ADS
    pub secondary_action: bool,
}

/// Camera toggle event - переключение между FPS и RTS camera
///
/// # Архитектура
/// - Emit: PlayerInputController при [V] key press
/// - Consume: camera_toggle_system (ECS)
///
/// # Эффекты
/// - FPS → RTS: player camera.set_current(false), RTS camera.set_current(true), show head meshes
/// - RTS → FPS: RTS camera.set_current(false), player camera.set_current(true), hide head meshes
#[derive(Event, Debug, Clone, Copy)]
pub struct CameraToggleEvent;

/// Mouse look event - mouse movement для camera rotation
///
/// # Архитектура
/// - Emit: PlayerInputController в `unhandled_input()` (mouse motion)
/// - Consume: player_mouse_look system (ECS)
///
/// # Rotation
/// - `delta_x`: horizontal mouse delta (pixels) → rotate Actor body (yaw Y)
/// - `delta_y`: vertical mouse delta (pixels) → rotate CameraPivot (pitch X, clamped)
///
/// # Pitch Limits
/// - Up: +89° (почти вертикаль вверх)
/// - Down: -30° (до груди)
#[derive(Event, Debug, Clone, Copy)]
pub struct MouseLookEvent {
    /// Horizontal mouse delta (pixels)
    pub delta_x: f32,

    /// Vertical mouse delta (pixels)
    pub delta_y: f32,
}

/// Weapon switch event - переключение оружия через hotkeys (1-9)
///
/// # Архитектура
/// - Emit: PlayerInputController при нажатии Digit1-9
/// - Consume: process_player_weapon_switch (ECS) → конвертирует в WeaponSwitchIntent
///
/// # Slot mapping
/// - Digit1 → slot_index = 0
/// - Digit2 → slot_index = 1
/// - ...
/// - Digit9 → slot_index = 8
#[derive(Event, Debug, Clone, Copy)]
pub struct WeaponSwitchEvent {
    /// Индекс слота (0-8)
    pub slot_index: u8,
}
