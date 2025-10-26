//! World positioning компоненты: StrategicPosition, PrefabPath

use bevy::prelude::*;

/// Strategic positioning (chunk-based, ECS authoritative)
///
/// ADR-005: Используется для AI decisions, saves, network sync.
/// Godot Transform — authoritative для physics/rendering.
/// Sync frequency: 0.1-1 Hz (zone transitions только).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct StrategicPosition {
    /// Chunk coordinates (32x32м grid)
    pub chunk: IVec2,
    /// Local offset внутри chunk (0-32 метров)
    pub local_offset: Vec2,
}

impl Default for StrategicPosition {
    fn default() -> Self {
        Self {
            chunk: IVec2::ZERO,
            local_offset: Vec2::ZERO,
        }
    }
}

impl StrategicPosition {
    /// Создать из world position (Vec3 → chunk + offset)
    pub fn from_world_position(pos: Vec3) -> Self {
        const CHUNK_SIZE: f32 = 32.0;

        let chunk_x = (pos.x / CHUNK_SIZE).floor() as i32;
        let chunk_z = (pos.z / CHUNK_SIZE).floor() as i32;

        let local_x = pos.x - (chunk_x as f32 * CHUNK_SIZE);
        let local_z = pos.z - (chunk_z as f32 * CHUNK_SIZE);

        Self {
            chunk: IVec2::new(chunk_x, chunk_z),
            local_offset: Vec2::new(local_x, local_z),
        }
    }

    /// Конвертировать в world position (для spawn в Godot)
    pub fn to_world_position(&self, y: f32) -> Vec3 {
        const CHUNK_SIZE: f32 = 32.0;

        let world_x = self.chunk.x as f32 * CHUNK_SIZE + self.local_offset.x;
        let world_z = self.chunk.y as f32 * CHUNK_SIZE + self.local_offset.y;

        Vec3::new(world_x, y, world_z)
    }
}

/// Prefab path for visual representation (data-driven)
///
/// ADR-007: TSCN prefabs для визуалов (Godot asset storage).
/// Позволяет разные визуалы для разных акторов.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct PrefabPath {
    pub path: String,
}

impl Default for PrefabPath {
    fn default() -> Self {
        Self {
            path: "res://actors/test_actor.tscn".to_string(),
        }
    }
}

impl PrefabPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}
