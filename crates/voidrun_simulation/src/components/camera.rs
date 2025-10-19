//! Camera mode tracking component
//!
//! Отмечает active camera mode для player-controlled entity.

use bevy::prelude::Component;

/// Camera mode (First-Person vs RTS)
///
/// Используется для toggle между FPS camera (attached to player head)
/// и RTS camera (strategic overview).
///
/// # Toggle
/// - [V] key → switch между режимами
/// - FPS mode: mouse captured, head meshes hidden
/// - RTS mode: mouse visible, head meshes shown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// First-person camera (attached to player head)
    FirstPerson,

    /// RTS camera (strategic overview)
    RTS,
}

/// Active camera mode component
///
/// Attached к player entity для tracking текущего camera mode.
///
/// # Usage
/// ```ignore
/// // Spawn player с FPS camera
/// commands.entity(player).insert(ActiveCamera {
///     mode: CameraMode::FirstPerson,
/// });
///
/// // Toggle
/// if let Ok(mut camera) = query.get_mut(player) {
///     camera.mode = match camera.mode {
///         CameraMode::FirstPerson => CameraMode::RTS,
///         CameraMode::RTS => CameraMode::FirstPerson,
///     };
/// }
/// ```
#[derive(Component, Debug, Clone, Copy)]
pub struct ActiveCamera {
    pub mode: CameraMode,
}

impl Default for ActiveCamera {
    fn default() -> Self {
        Self {
            mode: CameraMode::FirstPerson,
        }
    }
}
