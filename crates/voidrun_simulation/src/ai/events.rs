//! AI Events — события от Godot для AI систем
//!
//! Architecture: ADR-004 (Domain Events), ADR-005 (Godot Transform Ownership)
//! Godot VisionCone (Area3D) → GodotAIEvent → ECS AI FSM transitions

use bevy::prelude::*;

/// AI события от Godot (VisionCone callbacks)
///
/// Godot отправляет через Bevy Events когда:
/// - ActorSpotted: враг вошёл в VisionCone
/// - ActorLost: враг вышел из VisionCone
#[derive(Event, Debug, Clone)]
pub enum GodotAIEvent {
    /// Враг обнаружен (entered VisionCone)
    ActorSpotted {
        /// Entity наблюдателя (у кого VisionCone)
        observer: Entity,
        /// Entity цели (кого spotted)
        target: Entity,
    },

    /// Враг потерян (exited VisionCone или despawned)
    ActorLost {
        /// Entity наблюдателя
        observer: Entity,
        /// Entity цели
        target: Entity,
    },
}
