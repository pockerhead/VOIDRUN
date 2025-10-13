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
pub enum         GodotAIEvent {
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

/// Transform события от Godot (PostSpawn коррекция + движение)
///
/// ADR-005: Godot authoritative для Transform, ECS для StrategicPosition.
/// Event-driven sync вместо periodic polling.
#[derive(Event, Debug, Clone)]
pub enum GodotTransformEvent {
    /// PostSpawn: актор заспавнился в Godot, отправляем точную позицию для ECS коррекции
    PostSpawn {
        entity: Entity,
        position: Vec3, // Точная позиция после NavMesh placement
    },

    /// PositionChanged: актор двигался и изменил позицию (отправляется после move_and_slide)
    PositionChanged {
        entity: Entity,
        position: Vec3, // Новая позиция после движения
    },
}
