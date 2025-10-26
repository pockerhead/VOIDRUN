//! Decision execution logic.
//!
//! Chooses best action from available options and executes it (may cancel current actions).

use bevy::prelude::*;
use voidrun_simulation::combat::{MeleeAttackIntent, MeleeAttackState, MeleeAttackType, ParryDelayTimer};

use super::{ActionOption, ActionType, CurrentAction};
use voidrun_simulation::logger;
// ============================================================================
// Step 3: Choose Best Action
// ============================================================================

/// Choose best action from available options (highest priority).
///
/// Logs decision reasoning for debugging.
pub(super) fn choose_best_action(
    mut options: Vec<ActionOption>,
    current_action: &CurrentAction,
) -> ActionType {
    // Sort by priority (descending)
    options.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Log decision
    if let Some(best) = options.first() {
        logger::log(&format!(
            "🧠 AI decision: {:?} (priority: {:.2}, reason: {}, current: {:?})",
            best.action_type, best.priority, best.reason, current_action
        ));
    }

    // Return best action (or Wait if no options)
    options
        .into_iter()
        .next()
        .map(|o| o.action_type)
        .unwrap_or(ActionType::Wait)
}

// ============================================================================
// Step 4: Execute Decision
// ============================================================================

/// Execute decision (apply new action, cancel conflicting actions).
///
/// May interrupt current action if new action has higher priority.
pub(super) fn execute_decision(
    entity: Entity,
    decision: ActionType,
    current_action: CurrentAction,
    commands: &mut Commands,
    attack_intent_events: &mut EventWriter<MeleeAttackIntent>,
) {
    // First: Cancel conflicting current actions
    match current_action {
        CurrentAction::AttackWindup { interruptible, .. } if interruptible => {
            // Interrupt windup if starting new action
            if !matches!(decision, ActionType::Wait) {
                commands.entity(entity).remove::<MeleeAttackState>();
                logger::log(&format!(
                    "❌ AI: Cancelling own Windup (interruptible) for entity {:?}",
                    entity
                ));
            }
        }
        CurrentAction::PreparingParry { .. } => {
            // Cancel parry preparation if changing to attack
            if matches!(decision, ActionType::Attack { .. }) {
                commands.entity(entity).remove::<ParryDelayTimer>();
                logger::log(&format!(
                    "❌ AI: Cancelling parry preparation for entity {:?}",
                    entity
                ));
            }
        }
        _ => {}
    }

    // Second: Apply new action
    match decision {
        ActionType::Attack { target } => {
            attack_intent_events.write(MeleeAttackIntent {
                attacker: entity,
                attack_type: MeleeAttackType::Normal,
            });

            logger::log(&format!(
                "⚔️ AI: Entity {:?} decides to ATTACK target {:?}",
                entity, target
            ));
        }
        ActionType::Parry { attacker, delay } => {
            commands.entity(entity).insert(ParryDelayTimer::new(
                delay,
                attacker,
                delay + 0.1, // expected_windup_duration (delay + parry_windup)
            ));

            logger::log(&format!(
                "🛡️ AI: Entity {:?} decides to PARRY attacker {:?} (delay: {:.3}s)",
                entity, attacker, delay
            ));
        }
        ActionType::Wait => {
            // Do nothing
        }
    }
}
