//! Unified AI combat decision system.
//!
//! # Architecture
//!
//! **Problem:** Race condition between separate attack/parry decision systems.
//! - `ai_melee_attack_intent` generates attacks
//! - `ai_react_to_incoming_attacks` generates parries
//! → Result: Actor can start attack windup THEN decide to parry (conflicting states)
//!
//! **Solution:** Single decision system that:
//! - Evaluates ALL available actions (attack, parry, wait)
//! - Chooses best action by priority (based on AI behavior)
//! - Cancels conflicting current actions (e.g. interrupt windup to parry)
//!
//! # Decision Flow
//!
//! ```text
//! 1. get_current_action() → Idle | AttackWindup | ParryWindup | ...
//! 2. evaluate_available_actions() → [Attack(0.7), Parry(0.8), Wait(0.0)]
//! 3. choose_best_action() → Parry (highest priority)
//! 4. execute_decision() → Cancel Windup + Create ParryDelayTimer
//! ```
//!
//! # AI Behavior Priorities
//!
//! - **Aggressive**: Attack 0.7, Parry 0.6 (prefers offense)
//! - **Balanced**: Attack 0.5, Parry 0.8 (reactive)
//! - **Defensive**: Attack 0.3, Parry 0.95 (almost always parries)

use bevy::prelude::*;
use godot::prelude::*;
use rand::Rng;
use voidrun_simulation::ai::{AIState, GodotAIEvent};
use voidrun_simulation::combat::{
    AttackPhase, AttackType, MeleeAttackIntent, MeleeAttackState, MeleeAttackType, ParryDelayTimer,
    ParryState, StaggerState, WeaponStats,
};
use voidrun_simulation::components::{Stamina, Player};
use voidrun_simulation::Actor;

use crate::systems::VisualRegistry;
use crate::los_helpers::check_line_of_sight;

// ============================================================================
// Components
// ============================================================================

/// AI is waiting for opponent to attack (defensive tactic).
///
/// Blocks proactive attacks for duration, waiting for chance to parry.
/// Timer ticks down, allowing attack when expired or when opponent attacks first.
#[derive(Component, Debug, Clone)]
pub struct WaitingForOpening {
    /// Time remaining (seconds)
    pub timer: f32,
}

// ============================================================================
// Types: Current Action State
// ============================================================================

/// Current action state of an AI actor.
///
/// Determines which new actions are possible and whether current action can be interrupted.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CurrentAction {
    /// No active action
    Idle,

    /// Attacking: Windup phase
    ///
    /// `interruptible`: Can interrupt if progress < 50%
    /// `progress`: 0.0-1.0 (how far into windup)
    AttackWindup { interruptible: bool, progress: f32 },

    /// Attacking: Active phase (ActiveParryWindow or ActiveHitbox)
    ///
    /// Cannot interrupt - committed to attack
    AttackActive,

    /// Attacking: Recovery phase
    ///
    /// Vulnerable but can queue next action
    AttackRecovery,

    /// Preparing to parry (ParryDelayTimer ticking)
    ///
    /// `timer_remaining`: seconds until parry starts
    PreparingParry { timer_remaining: f32 },

    /// Parrying: Windup phase
    ///
    /// Cannot interrupt - committed to parry
    ParryWindup,

    /// Parrying: Recovery phase
    ///
    /// Cannot interrupt - finishing parry
    ParryRecovery,

    /// Staggered (stunned after being parried)
    ///
    /// Cannot take any actions
    Staggered,
}

// ============================================================================
// Types: Action Decisions
// ============================================================================

/// Type of action AI can take.
#[derive(Debug, Clone)]
enum ActionType {
    /// Melee attack target entity
    Attack { target: Entity },

    /// Parry incoming attack from attacker
    ///
    /// `delay`: seconds to wait before starting parry (AI reaction time)
    Parry { attacker: Entity, delay: f32 },

    /// Do nothing (default fallback)
    Wait,
}

/// Action option with priority score.
///
/// AI chooses action with highest priority.
#[derive(Debug, Clone)]
struct ActionOption {
    /// Type of action
    action_type: ActionType,

    /// Priority score (0.0-1.0, higher = more important)
    priority: f32,

    /// Debug reason for this action
    reason: &'static str,
}

// ============================================================================
// Main System: Unified AI Combat Decision
// ============================================================================

/// System: Unified AI **melee** combat decision making.
///
/// **Scope:** MELEE combat only (MeleeAttackIntent, ParryIntent)
/// **Note:** Ranged combat uses separate system (ai_weapon_fire_intent in ECS layer)
///
/// Runs on main thread (requires Godot node access for facing/distance validation).
///
/// # Two-Phase Decision Process
///
/// **Phase 1: Reactive (Parry Decision)**
/// - Triggered by `CombatAIEvent::EnemyAttackTelegraphed`
/// - Defender decides: parry incoming attack OR continue current action
/// - Can interrupt own AttackWindup to parry (if critical)
///
/// **Phase 2: Proactive (Attack Decision)**
/// - Runs for all AI in Combat state (every frame)
/// - Decides: attack target OR wait for opening
/// - Considers opponent's current action (attacking/parrying/idle)
///
/// # Interrupt Rules
///
/// - **Can interrupt AttackWindup** (if progress < 50%) to parry
/// - **Cannot interrupt AttackActive/ParryWindup** (committed)
/// - **Can start new attack after AttackRecovery** (cooldown permitting)
pub fn ai_melee_combat_decision_main_thread(
    mut telegraph_events: EventReader<GodotAIEvent>,
    ai_query: Query<(Entity, &AIState, &WeaponStats, &Stamina, &Actor), (Without<StaggerState>, Without<Player>)>,
    actor_query: Query<&Actor>,
    attacks: Query<&MeleeAttackState>,
    parries: Query<&ParryState>,
    delay_timers: Query<&ParryDelayTimer>,
    mut waiting_query: Query<(Entity, &mut WaitingForOpening)>,
    visuals: NonSend<VisualRegistry>,
    scene_root: NonSend<crate::systems::SceneRoot>,
    mut commands: Commands,
    mut attack_intent_events: EventWriter<MeleeAttackIntent>,
    time: Res<crate::systems::GodotDeltaTime>,
) {
    use std::collections::HashMap;

    let delta = time.0;

    // ========================================================================
    // STEP 0: Tick WaitingForOpening timers
    // ========================================================================
    let mut expired_waits = Vec::new();
    let mut updated_waits = Vec::new();

    for (entity, mut waiting) in waiting_query.iter_mut() {
        waiting.timer -= delta;

        if waiting.timer <= 0.0 {
            // Timer expired → mark for removal
            expired_waits.push(entity);
        } else {
            // Update timer
            updated_waits.push((entity, waiting));
        }
    }

    // Apply changes
    for entity in expired_waits {
        commands.entity(entity).remove::<WaitingForOpening>();
        voidrun_simulation::log(&format!(
            "⏰ AI: Entity {:?} finished waiting, can attack now",
            entity
        ));
    }


    // ========================================================================
    // STEP 1: Collect incoming attack telegraphs into HashMap (O(n))
    // ========================================================================
    let mut telegraphs: HashMap<Entity, (Entity, AttackType, f32)> = HashMap::new();

    for event in telegraph_events.read() {
        let GodotAIEvent::EnemyWindupVisible {
            attacker,
            defender,
            attack_type,
            windup_remaining,
        } = event
        else {
            continue;
        };

        // Store latest telegraph for each defender (if multiple attackers, last one wins)
        telegraphs.insert(*defender, (*attacker, attack_type.clone(), *windup_remaining));
    }

    // ========================================================================
    // STEP 2: Process all AI in Combat state (O(n) with O(1) HashMap lookup)
    // ========================================================================
    for (entity, ai_state, weapon, stamina, actor) in ai_query.iter() {
        // Only process AI in Combat state
        let AIState::Combat { target } = ai_state else {
            continue;
        };

        // Check if this entity has incoming attack telegraph
        if let Some((attacker, attack_type, windup_remaining)) = telegraphs.get(&entity) {
            // ================================================================
            // REACTIVE PATH: React to incoming attack telegraph
            // ================================================================
            react_to_incoming_attack(
                entity,
                *attacker,
                attack_type.clone(),
                *windup_remaining,
                ai_state,
                weapon,
                stamina,
                &attacks,
                &parries,
                &delay_timers,
                &visuals,
                &scene_root,
                &mut commands,
                &mut attack_intent_events,
            );
        } else {
            // ================================================================
            // PROACTIVE PATH: No incoming threat, decide to attack or wait
            // ================================================================
            // Skip if waiting for opening
            if waiting_query.get(entity).is_ok() {
                // Already waiting for opponent to attack
                continue;
            }

            proactive_attack_decision(
                entity,
                *target,
                actor,
                weapon,
                stamina,
                &actor_query,
                &attacks,
                &parries,
                &delay_timers,
                &visuals,
                &scene_root,
                &mut commands,
                &mut attack_intent_events,
            );
        }
    }
}

// ============================================================================
// Reactive Path: React to Incoming Attack
// ============================================================================

/// React to incoming attack telegraph (parry decision).
///
/// Evaluates all available actions (attack, parry, wait) and chooses best by priority.
fn react_to_incoming_attack(
    defender: Entity,
    attacker: Entity,
    attack_type: AttackType,
    windup_remaining: f32,
    ai_state: &AIState,
    weapon: &WeaponStats,
    stamina: &Stamina,
    attacks: &Query<&MeleeAttackState>,
    parries: &Query<&ParryState>,
    delay_timers: &Query<&ParryDelayTimer>,
    visuals: &NonSend<VisualRegistry>,
    scene_root: &NonSend<crate::systems::SceneRoot>,
    commands: &mut Commands,
    attack_intent_events: &mut EventWriter<MeleeAttackIntent>,
) {
    // 0. Cancel WaitingForOpening if present (got what we waited for!)
    commands.entity(defender).remove::<WaitingForOpening>();

    // 1. Analyze current action state
    let current_action = get_current_action(defender, attacks, parries, delay_timers);

    voidrun_simulation::log(&format!(
        "🧠 REACTIVE: entity {:?} reacting to attack from {:?}, current={:?}",
        defender, attacker, current_action
    ));

    // 2. Evaluate available actions (attack/parry/wait)
    let available_actions = evaluate_available_actions(
        defender,
        ai_state,
        weapon,
        stamina,
        &current_action,
        attacks,
        attacker,
        attack_type,
        windup_remaining,
        visuals,
    );

    // 3. Choose best action (highest priority)
    let decision = choose_best_action(available_actions, &current_action);

    // 4. Execute decision (may cancel current action)
    execute_decision(
        defender,
        decision,
        current_action,
        commands,
        attack_intent_events,
    );
}

// ============================================================================
// Proactive Path: Attack Decision
// ============================================================================

/// Proactive attack decision (no incoming threat).
///
/// AI decides to:
/// - **Attack** (aggressive) OR
/// - **Wait for opening** (defensive, wait for opponent to attack first)
///
/// Randomized decision based on strategy.
fn proactive_attack_decision(
    entity: Entity,
    target: Entity,
    entity_actor: &Actor,
    weapon: &WeaponStats,
    stamina: &Stamina,
    actor_query: &Query<&Actor>,
    attacks: &Query<&MeleeAttackState>,
    parries: &Query<&ParryState>,
    delay_timers: &Query<&ParryDelayTimer>,
    visuals: &NonSend<VisualRegistry>,
    scene_root: &NonSend<crate::systems::SceneRoot>,
    commands: &mut Commands,
    attack_intent_events: &mut EventWriter<MeleeAttackIntent>,
) {
    // 1. Analyze current action state
    let current_action = get_current_action(entity, attacks, parries, delay_timers);

    // Skip if already taking action (attacking, parrying, preparing)
    match current_action {
        CurrentAction::Idle | CurrentAction::AttackRecovery => {
            // Can decide to attack or wait
        }
        _ => {
            // Already busy (attacking, parrying, preparing)
            return;
        }
    }

    // 2. Friendly Fire Check: Не атаковать союзников (same faction_id)
    let Ok(target_actor) = actor_query.get(target) else {
        voidrun_simulation::log(&format!(
            "⚠️ PROACTIVE: entity {:?} cannot attack target {:?} (no Actor component)",
            entity, target
        ));
        return;
    };

    if target_actor.faction_id == entity_actor.faction_id {
        voidrun_simulation::log(&format!(
            "🚫 FRIENDLY FIRE PREVENTED: entity {:?} (faction {}) skips attack on ally {:?} (faction {})",
            entity, entity_actor.faction_id, target, target_actor.faction_id
        ));
        return;
    }

    // 3. Line-of-Sight Check: Не атаковать если LOS blocked
    // NOTE: movement_system.rs обработает LOS clearing через NavigationAgent
    match check_line_of_sight(entity, target, visuals, scene_root) {
        Some(true) => {
            // LOS clear → can attack
        }
        Some(false) => {
            // LOS blocked → пусть movement_system обходит через NavigationAgent
            voidrun_simulation::log(&format!(
                "🚫 LOS BLOCKED: entity {:?} → target {:?} (movement_system will handle pathfinding)",
                entity, target
            ));
            return;
        }
        None => {
            // Raycast failed (missing nodes?) → skip attack
            voidrun_simulation::log(&format!(
                "⚠️ PROACTIVE: entity {:?} LOS check failed for target {:?}",
                entity, target
            ));
            return;
        }
    }

    // 4. Check if can attack (stamina, cooldown)
    const ATTACK_COST: f32 = 30.0;
    if stamina.current < ATTACK_COST {
        return;
    }

    if !weapon.can_attack() {
        return;
    }

    // 5. Random decision: Attack (60%) vs Wait for Opening (40%)
    let should_attack = rand::thread_rng().gen_bool(0.6);

    if should_attack {
        // ========================================
        // ATTACK: Generate attack intent
        // ========================================
        attack_intent_events.write(MeleeAttackIntent {
            attacker: entity,
            attack_type: MeleeAttackType::Normal,
        });

        voidrun_simulation::log(&format!(
            "⚔️ PROACTIVE: entity {:?} decides to ATTACK (LOS clear, different faction)",
            entity
        ));
    } else {
        // ========================================
        // WAIT: Add WaitingForOpening component
        // ========================================
        let wait_duration = rand::thread_rng().gen_range(0.5..2.0); // 0.5-2.0 seconds

        commands.entity(entity).insert(WaitingForOpening {
            timer: wait_duration,
        });

        voidrun_simulation::log(&format!(
            "🧘 PROACTIVE: entity {:?} decides to WAIT for opening ({:.2}s)",
            entity, wait_duration
        ));
    }
}

// ============================================================================
// Step 1: Analyze Current Action
// ============================================================================

/// Determine actor's current action state.
///
/// Checks components: MeleeAttackState, ParryState, ParryDelayTimer.
fn get_current_action(
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
fn evaluate_available_actions(
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
    if !is_facing_attacker(&defender_node, &attacker_node) {
        voidrun_simulation::log(&format!(
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
        voidrun_simulation::log(&format!(
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

    voidrun_simulation::log(&format!(
        "🛡️ AI: Parry option available (defender: {:?}, attacker: {:?}, distance: {:.2}m, priority: {:.2})",
        defender, attacker, distance, priority
    ));

    Some(ActionOption {
        action_type: ActionType::Parry { attacker, delay },
        priority,
        reason: "incoming attack detected",
    })
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
    dot > 0.5
}

// ============================================================================
// Step 3: Choose Best Action
// ============================================================================

/// Choose best action from available options (highest priority).
///
/// Logs decision reasoning for debugging.
fn choose_best_action(
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
        voidrun_simulation::log(&format!(
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
fn execute_decision(
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
                voidrun_simulation::log(&format!(
                    "❌ AI: Cancelling own Windup (interruptible) for entity {:?}",
                    entity
                ));
            }
        }
        CurrentAction::PreparingParry { .. } => {
            // Cancel parry preparation if changing to attack
            if matches!(decision, ActionType::Attack { .. }) {
                commands.entity(entity).remove::<ParryDelayTimer>();
                voidrun_simulation::log(&format!(
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

            voidrun_simulation::log(&format!(
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

            voidrun_simulation::log(&format!(
                "🛡️ AI: Entity {:?} decides to PARRY attacker {:?} (delay: {:.3}s)",
                entity, attacker, delay
            ));
        }
        ActionType::Wait => {
            // Do nothing
        }
    }
}
