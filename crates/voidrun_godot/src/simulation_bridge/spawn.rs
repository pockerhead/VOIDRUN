//! Spawn helpers для создания entities
//!
//! Helpers для спавна NPC с разными конфигурациями (melee/ranged, factions, etc).

use bevy::prelude::{Commands, Entity, Vec3};
use voidrun_simulation::*;

/// Спавн melee NPC с мечом (для melee combat тестов)
pub fn spawn_melee_npc(
    commands: &mut Commands,
    position: (f32, f32, f32),
    faction_id: u64,
    max_hp: u32,
) -> Entity {
    let world_pos = Vec3::new(position.0, position.1, position.2);
    let strategic_pos = StrategicPosition::from_world_position(world_pos);

    commands
        .spawn((
            Actor { faction_id },
            strategic_pos,
            PrefabPath::new("res://actors/test_actor.tscn"),
            Health {
                current: max_hp,
                max: max_hp,
            },
            Stamina {
                current: 100.0,
                max: 100.0,
                regen_rate: 100.0, // 10x faster for testing combat
            },
            combat::WeaponStats::melee_sword(), // ✅ Melee weapon (sword)
            MovementCommand::Idle,
            NavigationState::default(),
            ai::AIState::Idle,
            ai::AIConfig {
                retreat_stamina_threshold: 0.2,
                retreat_health_threshold: 0.0,
                retreat_duration: 1.5,
                patrol_direction_change_interval: 3.0,
            },
            ai::SpottedEnemies::default(),
            Attachment {
                prefab_path: "res://actors/test_sword.tscn".to_string(), // ✅ Sword prefab
                attachment_point: "%RightHandAttachment".to_string(),
                attachment_type: AttachmentType::Weapon,
            },
        ))
        .id()
}

/// Спавн тестового NPC в ECS world (ADR-005: StrategicPosition + PrefabPath)
pub fn spawn_test_npc(
    commands: &mut Commands,
    position: (f32, f32, f32), // World position (будет конвертирован в StrategicPosition)
    faction_id: u64,
    max_hp: u32,
) -> Entity {
    let world_pos = Vec3::new(position.0, position.1, position.2);
    let strategic_pos = StrategicPosition::from_world_position(world_pos);

    commands
        .spawn((
            Actor { faction_id },
            strategic_pos, // StrategicPosition (sync_strategic_position_from_godot обновит из Godot)
            PrefabPath::new("res://actors/test_actor.tscn"), // Data-driven prefab path
            Health {
                current: max_hp,
                max: max_hp,
            },
            Stamina {
                current: 100.0,
                max: 100.0,
                regen_rate: 10.0, // 10 stamina/sec
            },
            combat::WeaponStats::ranged_pistol(), // Unified weapon stats (ranged)
            MovementCommand::Idle,                // Godot будет читать и выполнять
            NavigationState::default(), // Трекинг достижения navigation target (для PositionChanged events)
            ai::AIState::Idle,
            ai::AIConfig {
                retreat_stamina_threshold: 0.2,        // Retreat при stamina < 20%
                retreat_health_threshold: 0.0,         // Retreat при HP < 10% (было 20%)
                retreat_duration: 1.5,                 // Быстрее возвращаются в бой
                patrol_direction_change_interval: 3.0, // Каждые 3 сек новое направление
            },
            ai::SpottedEnemies::default(), // Godot VisionCone → GodotAIEvent → обновляет список
            Attachment {
                prefab_path: "res://actors/test_pistol.tscn".to_string(),
                attachment_point: "%RightHandAttachment".to_string(),
                attachment_type: AttachmentType::Weapon,
            },
        ))
        .id()
}
