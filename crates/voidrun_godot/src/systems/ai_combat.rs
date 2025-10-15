//! AI combat decision system (Godot tactical layer).
//!
//! # Architecture
//!
//! **ECS (Strategic Layer):**
//! - `CombatAIEvent::EnemyAttackTelegraphed`: Enemy started attack windup
//! - AI decides to parry based on:
//!   - Combat state (must be in Combat, not Idle/Patrolling)
//!   - Facing check (attacker must be in front, not behind)
//!   - Distance check (not too far away)
//!   - Reaction time (AI skill-based)
//!
//! **Godot (Tactical Layer):**
//! - Facing validation (uses Godot Transform for accurate direction)
//! - Distance validation (uses Godot GlobalPosition)
//!
//! # Flow
//!
//! ```text
//! ECS: EnemyAttackTelegraphed (windup started)
//!   ↓
//! Godot: ai_react_to_incoming_attacks_main_thread
//!   ↓ (validate Combat state, facing, distance)
//! ECS: ParryIntent event
//!   ↓
//! ECS: start_parry system → adds ParryState
//! ```

use bevy::prelude::*;
use godot::prelude::*;
use voidrun_simulation::ai::{AIState, CombatAIEvent};
use voidrun_simulation::combat::{MeleeAttackState, ParryIntent, WeaponStats};

use crate::systems::VisualRegistry;

/// System: AI reacts to incoming attacks (Godot tactical validation).
///
/// Validates:
/// - Combat state (defender must be in Combat AI state)
/// - Facing check (attacker must be in front ~60° cone)
/// - Distance check (not too far for parry)
///
/// If validation passes → generates `ParryIntent` event.
///
/// **Runs on main thread** (requires Godot node access via VisualRegistry).
pub fn ai_react_to_incoming_attacks_main_thread(
    mut telegraph_events: EventReader<CombatAIEvent>,
    query: Query<(&AIState, &WeaponStats), Without<MeleeAttackState>>,
    visuals: NonSend<VisualRegistry>,
    mut commands: Commands,
) {
    for event in telegraph_events.read() {
        let CombatAIEvent::EnemyAttackTelegraphed {
            attacker,
            target: defender,
            attack_type: _,
            windup_remaining,
        } = event else {
            continue;
        };

        // Get defender components
        let Ok((defender_ai, defender_weapon)) = query.get(*defender) else {
            continue;
        };

        // 1. Validate Combat state
        let AIState::Combat { .. } = defender_ai else {
            voidrun_simulation::log(&format!(
                "❌ AI: Defender {:?} not in Combat state (cannot parry)",
                defender
            ));
            continue;
        };

        // 2. Get Godot nodes for facing/distance check
        let Some(defender_node_3d) = visuals.visuals.get(defender) else {
            continue;
        };
        let Some(attacker_node_3d) = visuals.visuals.get(attacker) else {
            continue;
        };

        // Cast to CharacterBody3D for API access
        let Ok(defender_node) = defender_node_3d.clone().try_cast::<godot::classes::CharacterBody3D>() else {
            continue;
        };
        let Ok(attacker_node) = attacker_node_3d.clone().try_cast::<godot::classes::CharacterBody3D>() else {
            continue;
        };

        // 3. Facing check: attacker must be in front of defender
        if !is_facing_attacker(&defender_node, &attacker_node) {
            voidrun_simulation::log(&format!(
                "❌ AI: Defender {:?} cannot parry - attacker {:?} is behind/side",
                defender, attacker
            ));
            continue;
        };

        // 4. Distance check: not too far for melee parry
        let distance = defender_node
            .get_global_position()
            .distance_to(attacker_node.get_global_position());

        const MAX_PARRY_DISTANCE: f32 = 3.0; // meters
        if distance > MAX_PARRY_DISTANCE {
            voidrun_simulation::log(&format!(
                "❌ AI: Defender {:?} cannot parry - attacker {:?} too far ({:.2}m > {:.2}m)",
                defender, attacker, distance, MAX_PARRY_DISTANCE
            ));
            continue;
        }

        // 5. Decision: should parry? (random strategy: 50% parry, 50% aggressive)
        if should_parry(*windup_remaining, defender_weapon) {
            // Random strategy: 50% chance to parry (defensive) vs 50% keep attacking (aggressive)
            use rand::Rng;
            let defensive_strategy = rand::thread_rng().gen_bool(0.5);

            if defensive_strategy {
                // Calculate delay: aim to finish parry windup when attacker reaches ActiveParryWindow
                // Parry windup = 0.1s, so start parrying at: windup_remaining - 0.1s + margin
                let parry_windup = 0.1;
                let margin = rand::thread_rng().gen_range(-0.05..0.05); // ±50ms error
                let delay = (*windup_remaining - parry_windup + margin).max(0.0);

                // Add ParryDelayTimer component (ECS will tick it and generate ParryIntent)
                commands.entity(*defender).insert(
                    voidrun_simulation::combat::ParryDelayTimer::new(
                        delay,
                        *attacker,
                        *windup_remaining
                    )
                );

                voidrun_simulation::log(&format!(
                    "🛡️ AI: Defender {:?} decides to PARRY attacker {:?} (distance: {:.2}m, windup: {:.2}s, delay: {:.3}s, margin: {:.3}s)",
                    defender, attacker, distance, windup_remaining, delay, margin
                ));
            } else {
                voidrun_simulation::log(&format!(
                    "⚔️ AI: Defender {:?} chooses AGGRESSIVE strategy (keeps attacking instead of parrying)",
                    defender
                ));
            }
        } else {
            voidrun_simulation::log(&format!(
                "⏳ AI: Defender {:?} IGNORES parry (not enough time/stamina)",
                defender
            ));
        }
    }
}

/// Check if defender is facing attacker (front 60° cone).
///
/// Returns true if attacker is in front of defender (dot product > 0.5).
fn is_facing_attacker(
    defender_node: &Gd<godot::classes::CharacterBody3D>,
    attacker_node: &Gd<godot::classes::CharacterBody3D>,
) -> bool {
    let defender_pos = defender_node.get_global_position();
    let attacker_pos = attacker_node.get_global_position();

    let to_attacker = (attacker_pos - defender_pos).normalized();

    // Godot forward = -Z axis (Transform basis column C)
    let defender_forward = -defender_node.get_global_transform().basis.col_c();

    let dot = to_attacker.dot(defender_forward);

    // dot > 0.5 means ~60° cone in front
    // dot > 0.0 means ~90° cone (too wide for parry)
    dot > 0.5
}

/// Decide if AI should attempt parry.
///
/// Conditions:
/// - Enough time to react (windup_remaining >= reaction_time)
/// - Weapon supports parry (parry_active_duration > 0)
///
/// TODO: Add skill-based randomness (AICombatProfile component).
fn should_parry(windup_remaining: f32, weapon: &WeaponStats) -> bool {
    // Reaction time: AI needs at least 0.2s to react
    const AI_REACTION_TIME: f32 = 0.2;

    if windup_remaining < AI_REACTION_TIME {
        return false; // Too late to react
    }

    // Check if weapon can parry
    if !weapon.can_parry() {
        return false;
    }

    // TODO: Add skill-based decision making:
    // - AICombatProfile.parry_chance (0.2 rookie - 0.8 veteran)
    // - Random roll for decision
    // For now: always parry if conditions met
    true
}
