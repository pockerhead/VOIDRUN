//! Action evaluation logic.
//!
//! Evaluates available actions (attack, parry, wait) based on current state and tactical constraints.

use bevy::prelude::*;
use rand::Rng;
use voidrun_simulation::ai::AIState;
use voidrun_simulation::combat::{
    AttackPhase, AttackType, MeleeAttackState, ParryState, ParryDelayTimer, WeaponStats,
};
use voidrun_simulation::components::Stamina;
use voidrun_simulation::logger;
use crate::shared::VisualRegistry;

use super::{ActionOption, ActionType, CurrentAction};

// ============================================================================
// Step 1: Analyze Current Action
// ============================================================================

/// Determine actor's current action state.
///
/// Checks components: MeleeAttackState, ParryState, ParryDelayTimer.
pub(super) fn get_current_action(
    entity: Entity,
    attacks: &Query<&MeleeAttackState>,
    parries: &Query<&ParryState>,
    delay_timers: &Query<&ParryDelayTimer>,
) -> CurrentAction {
    // Check if staggered (handled by query filter in main system)
    // Stagger is filtered out in ai_query (Without<StaggerState>)

    // Check if parrying
    if let Ok(parry_state) = parries.get(entity) {
        return match &parry_state.phase {
            voidrun_simulation::combat::ParryPhase::Windup { .. } => CurrentAction::ParryWindup,
            voidrun_simulation::combat::ParryPhase::Recovery { .. } => CurrentAction::ParryRecovery,
        };
    }

    // Check if preparing to parry
    if let Ok(delay_timer) = delay_timers.get(entity) {
        return CurrentAction::PreparingParry {
            timer_remaining: delay_timer.timer,
        };
    }

    // Check if attacking
    if let Ok(attack_state) = attacks.get(entity) {
        match &attack_state.phase {
            AttackPhase::Windup { duration } => {
                let progress = 1.0 - (attack_state.phase_timer / duration);
                let interruptible = progress < 0.5; // Can interrupt first 50% of windup
                return CurrentAction::AttackWindup {
                    interruptible,
                    progress,
                };
            }
            AttackPhase::ActiveParryWindow { .. } | AttackPhase::ActiveHitbox { .. } => {
                return CurrentAction::AttackActive;
            }
            AttackPhase::Recovery { .. } => {
                return CurrentAction::AttackRecovery;
            }
            AttackPhase::Idle => {}
        }
    }

    CurrentAction::Idle
}

// ============================================================================
// Step 2: Evaluate Available Actions
// ============================================================================

/// Evaluate all available actions (attack, parry, wait).
///
/// Returns list of ActionOption with priority scores.
pub(super) fn evaluate_available_actions(
    entity: Entity,
    ai_state: &AIState,
    weapon: &WeaponStats,
    stamina: &Stamina,
    current_action: &CurrentAction,
    attacks: &Query<&MeleeAttackState>,
    incoming_attacker: Entity,
    incoming_attack_type: AttackType,
    incoming_windup_remaining: f32,
    visuals: &NonSend<VisualRegistry>,
) -> Vec<ActionOption> {
    let mut options = Vec::new();

    // Evaluate attack option
    if can_attack(stamina, weapon, current_action) {
        if let AIState::Combat { target } = ai_state {
            if let Some(attack_option) = evaluate_attack_option(*target, ai_state) {
                options.push(attack_option);
            }
        }
    }

    // Evaluate parry option (against incoming attack)
    if can_parry(current_action) {
        if let Some(parry_option) = evaluate_parry_option(
            entity,
            ai_state,
            incoming_attacker,
            incoming_attack_type,
            incoming_windup_remaining,
            attacks,
            visuals,
        ) {
            options.push(parry_option);
        }
    }

    // Always have Wait as fallback
    options.push(ActionOption {
        action_type: ActionType::Wait,
        priority: 0.0,
        reason: "default fallback",
    });

    options
}

/// Check if actor can attack.
///
/// Requirements:
/// - Enough stamina
/// - No conflicting action (or action is interruptible)
fn can_attack(stamina: &Stamina, weapon: &WeaponStats, current_action: &CurrentAction) -> bool {
    // Check stamina
    const ATTACK_COST: f32 = 30.0;
    if stamina.current < ATTACK_COST {
        return false;
    }

    // Check weapon cooldown
    if !weapon.can_attack() {
        return false;
    }

    // Check current action allows attacking
    match current_action {
        CurrentAction::Idle => true,
        CurrentAction::AttackRecovery => true, // Can queue next attack
        CurrentAction::AttackWindup { interruptible, .. } => *interruptible, // Can interrupt early windup
        _ => false,
    }
}

/// Check if actor can parry.
///
/// Requirements:
/// - No conflicting action (or action is interruptible)
fn can_parry(current_action: &CurrentAction) -> bool {
    match current_action {
        CurrentAction::Idle => true,
        CurrentAction::AttackWindup { interruptible, .. } => *interruptible, // Can cancel windup to parry
        CurrentAction::AttackRecovery => true, // Can parry after attack
        _ => false,
    }
}

/// Evaluate attack action option.
///
/// Returns ActionOption with priority based on AI behavior.
fn evaluate_attack_option(target: Entity, ai_state: &AIState) -> Option<ActionOption> {
    // Determine priority based on AI behavior
    // TODO: When AIBehavior is implemented, use actual behavior
    // For now: use 50/50 random strategy (50% aggressive, 50% defensive)
    let aggressive_strategy = rand::thread_rng().gen_bool(0.5);

    let priority = if aggressive_strategy { 0.7 } else { 0.3 };

    Some(ActionOption {
        action_type: ActionType::Attack { target },
        priority,
        reason: "target in range",
    })
}

/// Evaluate parry action option (with Godot facing/distance validation).
///
/// Returns ActionOption with priority if parry is viable, None otherwise.
fn evaluate_parry_option(
    defender: Entity,
    ai_state: &AIState,
    attacker: Entity,
    attack_type: AttackType,
    windup_remaining: f32,
    attacks: &Query<&MeleeAttackState>,
    visuals: &NonSend<VisualRegistry>,
) -> Option<ActionOption> {
    // Future: Check if attack is parryable based on type
    // if attack_type == AttackType::Heavy { return None; }  // Heavy cannot be parried

    // For now: all attacks can be parried
    // 1. Check attacker is in Windup phase (can react to)
    let Ok(attack_state) = attacks.get(attacker) else {
        return None;
    };

    if !matches!(attack_state.phase, AttackPhase::Windup { .. }) {
        return None;
    }

    // 2. Check reaction time (need at least 0.2s to react)
    const AI_REACTION_TIME: f32 = 0.2;
    if windup_remaining < AI_REACTION_TIME {
        return None;
    }

    // 3. Get Godot nodes for facing/distance validation
    let Some(defender_node_3d) = visuals.visuals.get(&defender) else {
        return None;
    };
    let Some(attacker_node_3d) = visuals.visuals.get(&attacker) else {
        return None;
    };

    // Cast to CharacterBody3D for API access
    let Ok(defender_node) = defender_node_3d
        .clone()
        .try_cast::<godot::classes::CharacterBody3D>()
    else {
        return None;
    };
    let Ok(attacker_node) = attacker_node_3d
        .clone()
        .try_cast::<godot::classes::CharacterBody3D>()
    else {
        return None;
    };

    // 4. Facing check: attacker must be in front of defender
    if !super::validation::is_facing_attacker(&defender_node, &attacker_node) {
        logger::log(&format!(
            "❌ AI: Defender {:?} cannot parry - attacker {:?} is behind/side",
            defender, attacker
        ));
        return None;
    }

    // 5. Distance check: not too far for melee parry
    let distance = defender_node
        .get_global_position()
        .distance_to(attacker_node.get_global_position());

    const MAX_PARRY_DISTANCE: f32 = 3.0; // meters
    if distance > MAX_PARRY_DISTANCE {
        logger::log(&format!(
            "❌ AI: Defender {:?} cannot parry - attacker {:?} too far ({:.2}m > {:.2}m)",
            defender, attacker, distance, MAX_PARRY_DISTANCE
        ));
        return None;
    }

    // 6. Calculate delay for parry timing
    let parry_windup = 0.1;
    let margin = rand::thread_rng().gen_range(-0.05..0.05); // ±50ms error
    let delay = (windup_remaining - parry_windup + margin).max(0.0);

    // 7. Determine priority based on AI behavior
    // TODO: When AIBehavior is implemented, use actual behavior
    // For now: use 50/50 random strategy (50% aggressive, 50% defensive)
    let defensive_strategy = rand::thread_rng().gen_bool(0.5);

    let priority = if defensive_strategy { 0.8 } else { 0.6 };

    logger::log(&format!(
        "🛡️ AI: Parry option available (defender: {:?}, attacker: {:?}, distance: {:.2}m, priority: {:.2})",
        defender, attacker, distance, priority
    ));

    Some(ActionOption {
        action_type: ActionType::Parry { attacker, delay },
        priority,
        reason: "incoming attack detected",
    })
}
