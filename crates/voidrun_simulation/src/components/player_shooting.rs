//! Player Shooting Components - ADS and Hip Fire aiming modes
//!
//! Architecture:
//! - AimMode tracks player aim state (Hip Fire, ADS, transitions)
//! - Procedural positioning (NOT keyframe animations)
//! - RMB toggles between modes (smooth lerp transition)
//!
//! Flow:
//! 1. Player presses RMB → ToggleADSIntent event
//! 2. System processes intent → update AimMode
//! 3. Godot systems read AimMode → position RightHand procedurally

use bevy::prelude::*;

/// Player aiming mode state
///
/// Controls how weapon is positioned:
/// - **HipFire:** Weapon low, aims to raycast hit point (dynamic)
/// - **ADS:** Weapon raised, sight socket on camera ray (aligned)
/// - **Transitioning:** Smooth lerp between states (0.3s)
///
/// # Architecture Note
///
/// This component ТОЛЬКО для Player! AI actors используют weapon_aim_main_thread
/// для automatic targeting (не нужны ADS transitions).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub enum AimMode {
    /// Hip fire mode - weapon positioned low, aims to raycast hit point
    ///
    /// Hand position: dynamic based on camera raycast (50m max)
    /// - If raycast hits → aim to hit.position
    /// - If no hit → aim to camera_pos + forward * 50m
    HipFire,

    /// Transitioning from Hip Fire → ADS
    ///
    /// Smooth lerp (0.3s) from hip_fire_position → ads_target_position
    /// - start_position: RightHand position at transition start (world space)
    /// - progress: 0.0 → 1.0 (updated every frame)
    /// - Easing: ease_out_cubic for smooth deceleration
    EnteringADS {
        /// RightHand position when transition started (world space Vec3)
        start_position: Vec3,
        /// Transition progress (0.0 = start, 1.0 = complete)
        progress: f32,
    },

    /// Aim down sights mode - weapon raised, sight socket aligned with camera
    ///
    /// Hand position: calculated EVERY frame (camera can rotate!)
    /// - SightSocket на camera ray (sight_distance from camera)
    /// - Hand position = target_sight_pos - (weapon_rotation * sight_offset)
    /// - Weapon looks forward (camera direction)
    ADS,

    /// Transitioning from ADS → Hip Fire
    ///
    /// Smooth lerp (0.3s) from ads_position → hip_fire_position
    /// - Similar to EnteringADS but reverse direction
    ExitingADS {
        /// RightHand position when transition started (world space Vec3)
        start_position: Vec3,
        /// Transition progress (0.0 = start, 1.0 = complete)
        progress: f32,
    },
}

impl Default for AimMode {
    fn default() -> Self {
        Self::HipFire
    }
}

impl AimMode {
    /// Transition duration (seconds)
    ///
    /// 300ms - fast enough to feel responsive, slow enough to see animation
    pub const TRANSITION_DURATION: f32 = 0.3;

    /// Can player shoot in this mode?
    ///
    /// Blocked during transitions (prevent spam, tactical cost)
    pub fn can_shoot(&self) -> bool {
        matches!(self, AimMode::HipFire | AimMode::ADS)
    }

    /// Is player currently in ADS (fully or transitioning)?
    pub fn is_ads_or_entering(&self) -> bool {
        matches!(
            self,
            AimMode::ADS | AimMode::EnteringADS { .. } | AimMode::ExitingADS { .. }
        )
    }

    /// Is player fully in ADS (not transitioning)?
    pub fn is_fully_ads(&self) -> bool {
        matches!(self, AimMode::ADS)
    }
}

/// Event: Toggle ADS mode (RMB input)
///
/// Player presses RMB → toggle between Hip Fire ↔ ADS
///
/// # Behavior
///
/// - HipFire → start EnteringADS transition
/// - ADS → start ExitingADS transition
/// - Transitioning → ignore (prevent spam)
#[derive(Event, Debug, Clone)]
pub struct ToggleADSIntent {
    /// Player entity
    pub entity: Entity,
}

/// Helper: Ease-out cubic curve
///
/// Smooth deceleration: fast start, slow finish
/// - t=0.0 → 0.0
/// - t=0.5 → 0.875
/// - t=1.0 → 1.0
///
/// Formula: (t-1)³ + 1
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t - 1.0;
    t * t * t + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aim_mode_default() {
        let mode = AimMode::default();
        assert!(matches!(mode, AimMode::HipFire));
    }

    #[test]
    fn test_can_shoot() {
        assert!(AimMode::HipFire.can_shoot());
        assert!(AimMode::ADS.can_shoot());

        assert!(!AimMode::EnteringADS {
            start_position: Vec3::ZERO,
            progress: 0.5
        }
        .can_shoot());

        assert!(!AimMode::ExitingADS {
            start_position: Vec3::ZERO,
            progress: 0.5
        }
        .can_shoot());
    }

    #[test]
    fn test_is_ads_or_entering() {
        assert!(!AimMode::HipFire.is_ads_or_entering());
        assert!(AimMode::ADS.is_ads_or_entering());

        assert!(AimMode::EnteringADS {
            start_position: Vec3::ZERO,
            progress: 0.5
        }
        .is_ads_or_entering());

        assert!(AimMode::ExitingADS {
            start_position: Vec3::ZERO,
            progress: 0.5
        }
        .is_ads_or_entering());
    }

    #[test]
    fn test_ease_out_cubic() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);

        // Mid-point should be close to 0.875
        let mid = ease_out_cubic(0.5);
        assert!(mid > 0.8 && mid < 0.9);
    }
}
