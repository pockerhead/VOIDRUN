//! Player spawn helper
//!
//! Утилиты для spawn player entity в ECS world.

use bevy::prelude::*;
use voidrun_simulation::components::*;

/// Spawn player entity в ECS world
///
/// # Параметры
/// - `commands`: ECS Commands для spawn
/// - `position`: Starting position (world coordinates)
///
/// # Returns
/// Entity ID созданного player
///
/// # Компоненты
/// - Player marker (отличает от NPC)
/// - Actor (базовые характеристики)
/// - Health, Stamina
/// - WeaponStats (базовое melee оружие)
/// - Movement components (MovementCommand, NavigationState)
/// - AI components НЕ добавляются (player controlled, не AI)
/// - StrategicPosition (starting position)
/// - PrefabPath (для визуалов - пока test_actor.tscn, TODO: player-specific prefab)
///
/// # Future (save/load)
/// - Loadout (starting gear)
/// - Stats (level, skills)
/// - Inventory
pub fn spawn_player(
    commands: &mut Commands,
    position: Vec3,
) -> Entity {
    use voidrun_simulation::combat::WeaponStats;

    let strategic_pos = StrategicPosition::from_world_position(position);

    commands
        .spawn((
            Player, // Marker: player-controlled (не AI)
            Actor { faction_id: 0 }, // Faction 0 = player faction
            strategic_pos,
            PrefabPath::new("res://actors/test_actor.tscn"), // TODO: player-specific prefab (test_player.tscn)
            Health {
                current: 100,
                max: 100,
            },
            Stamina {
                current: 100.0,
                max: 100.0,
                regen_rate: 10.0, // 10 stamina/sec
            },
            WeaponStats::melee_sword(), // Starting weapon (melee sword)
            // НЕ добавляем MovementCommand - player управляется НАПРЯМУЮ через velocity (FPS-style)
            // НЕ добавляем NavigationState - player не использует NavigationAgent pathfinding
            // НЕ добавляем AIState, AIConfig, SpottedEnemies - это для NPC!
            // Player управляется через PlayerInputEvent → НАПРЯМУЮ CharacterBody3D velocity
            Attachment {
                prefab_path: "res://actors/test_sword.tscn".to_string(),
                attachment_point: "RightHand/WeaponAttachment".to_string(),
                attachment_type: AttachmentType::Weapon,
            },
        ))
        .id()
}
