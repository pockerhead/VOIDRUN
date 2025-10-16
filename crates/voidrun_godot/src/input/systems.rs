//! Player input systems (ECS)
//!
//! Обрабатывают PlayerInputEvent и НАПРЯМУЮ управляют CharacterBody3D velocity.
//!
//! # Архитектура
//!
//! **Player НЕ использует NavigationAgent/MovementCommand!**
//! - AI actors: Input → MovementCommand → NavigationAgent → velocity
//! - Player: Input → НАПРЯМУЮ CharacterBody3D velocity (FPS-style)
//!
//! **Почему:**
//! - Player не нуждается в pathfinding (мы управляем direction напрямую)
//! - Нет lag от NavigationAgent processing
//! - Прямой контроль = responsive gameplay

use bevy::prelude::*;
use voidrun_simulation::components::{Player, JumpIntent};
use voidrun_simulation::combat::{MeleeAttackIntent, MeleeAttackType};

use super::events::PlayerInputEvent;
use crate::systems::VisualRegistry;

/// Player movement system - НАПРЯМУЮ устанавливает velocity CharacterBody3D
///
/// # Архитектура
/// - Читает: PlayerInputEvent (from PlayerInputController)
/// - Пишет: CharacterBody3D.velocity (НАПРЯМУЮ через Godot API)
/// - Query: With<Player> (только player-controlled actors)
///
/// # Movement
/// - WASD → CharacterBody3D.velocity (FPS-style direct control)
/// - Sprint → speed multiplier (6.0 vs 3.0 м/с)
/// - Space → JumpIntent event (обрабатывается gravity system)
///
/// # Координаты
/// Input Vec2 (x, y) → World Vec3 (x, 0, z):
/// - Input.x = horizontal (left/right) → World X
/// - Input.y = vertical (forward/back) → World Z
/// - World Y = up (gravity axis, handled by gravity system)
///
/// # Важно
/// - НЕ используем MovementCommand (это для AI pathfinding)
/// - НЕ используем NavigationAgent (это для AI avoidance)
/// - Прямое управление velocity как в FPS играх
pub fn process_player_input(
    mut input_events: EventReader<PlayerInputEvent>,
    mut jump_events: EventWriter<JumpIntent>,
    player_query: Query<Entity, With<Player>>,
    visuals: NonSend<VisualRegistry>,
) {
    // Guard: нет player entity
    let Ok(player_entity) = player_query.single() else {
        return;
    };

    // Get Godot CharacterBody3D node
    let Some(player_node_3d) = visuals.visuals.get(&player_entity) else {
        return;
    };

    let Ok(mut player_body) = player_node_3d
        .clone()
        .try_cast::<godot::classes::CharacterBody3D>()
    else {
        return;
    };

    for input in input_events.read() {
        // WASD movement - НАПРЯМУЮ velocity
        if !input.move_direction.is_nan() && input.move_direction.length_squared() > 0.01 {
            let speed = if input.sprint { 6.0 } else { 3.0 }; // unlimited sprint

            // Convert input Vec2 to Godot Vector3 (XZ plane)
            // NOTE: Godot uses (x, y, z) where y = up
            let velocity = godot::prelude::Vector3::new(
                input.move_direction.x * speed,
                player_body.get_velocity().y, // Keep Y velocity (gravity handled by gravity system)
                input.move_direction.y * speed, // Forward/back
            );

            player_body.set_velocity(velocity);
        } else {
            // No movement input → stop horizontal movement (keep Y for gravity)
            let mut velocity = player_body.get_velocity();
            velocity.x = 0.0;
            velocity.z = 0.0;
            player_body.set_velocity(velocity);
        }

        // Jump
        if input.jump {
            jump_events.write(JumpIntent {
                entity: player_entity,
            });
        }
    }
    player_body.move_and_slide();
}

/// Player combat input system - обрабатывает attack/parry input
///
/// # Архитектура
/// - Читает: PlayerInputEvent
/// - Пишет: MeleeAttackIntent event
/// - Query: With<Player>
///
/// # Combat
/// - LMB → MeleeAttackIntent (нужен target - пока None, TODO: raycasting)
/// - RMB → ParryIntent (TODO: когда реализуем parry system)
///
/// # TODO
/// - [ ] Target selection (raycast от camera или closest enemy)
/// - [ ] Parry intent когда реализуем parry
pub fn player_combat_input(
    mut input_events: EventReader<PlayerInputEvent>,
    mut attack_events: EventWriter<MeleeAttackIntent>,
    player_query: Query<Entity, With<Player>>,
) {
    // Guard: нет player entity
    let Ok(player_entity) = player_query.single() else {
        return;
    };

    for input in input_events.read() {
        // LMB → Melee Attack
        // TODO: Временно закомментировано - заработает после Фазы 3 рефакторинга
        // (MeleeAttackIntent.target будет убран, area-based detection)
        if input.attack {
            voidrun_simulation::log("⚠️ Player attack pressed - combat will be available after Phase 3 refactor");
            // attack_events.write(MeleeAttackIntent {
            //     attacker: player_entity,
            //     attack_type: MeleeAttackType::Normal,
            // });
        }

        // RMB → Parry (TODO)
        // if input.parry {
        //     parry_events.send(ParryIntent { ... });
        // }
    }
}
