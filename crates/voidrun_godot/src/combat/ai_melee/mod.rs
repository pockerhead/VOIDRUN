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
use rand::Rng;
use voidrun_simulation::ai::{AIState, GodotAIEvent};
use voidrun_simulation::combat::{
    AttackType, MeleeAttackIntent, MeleeAttackState, MeleeAttackType, ParryDelayTimer,
    ParryState, StaggerState, WeaponStats,
};
use voidrun_simulation::{Stamina, Actor};
use voidrun_simulation::player::Player;
use voidrun_simulation::logger;

use crate::shared::VisualRegistry;
use crate::shared::los_helpers::check_line_of_sight;

// Submodules
mod evaluation;
mod decision;
mod validation;

// Re-export key functions
use evaluation::{evaluate_available_actions, get_current_action};
use decision::{choose_best_action, execute_decision};

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
pub(super) enum CurrentAction {
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
pub(super) enum ActionType {
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
pub(super) struct ActionOption {
    /// Type of action
    pub(super) action_type: ActionType,

    /// Priority score (0.0-1.0, higher = more important)
    pub(super) priority: f32,

    /// Debug reason for this action
    pub(super) reason: &'static str,
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
    scene_root: NonSend<crate::shared::SceneRoot>,
    mut commands: Commands,
    mut attack_intent_events: EventWriter<MeleeAttackIntent>,
    time: Res<crate::shared::GodotDeltaTime>,
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
        logger::log(&format!(
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
    scene_root: &NonSend<crate::shared::SceneRoot>,
    commands: &mut Commands,
    attack_intent_events: &mut EventWriter<MeleeAttackIntent>,
) {
    // 0. Cancel WaitingForOpening if present (got what we waited for!)
    commands.entity(defender).remove::<WaitingForOpening>();

    // 1. Analyze current action state
    let current_action = get_current_action(defender, attacks, parries, delay_timers);

    logger::log(&format!(
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
    scene_root: &NonSend<crate::shared::SceneRoot>,
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
        logger::log(&format!(
            "⚠️ PROACTIVE: entity {:?} cannot attack target {:?} (no Actor component)",
            entity, target
        ));
        return;
    };

    if target_actor.faction_id == entity_actor.faction_id {
        logger::log(&format!(
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
            logger::log(&format!(
                "🚫 LOS BLOCKED: entity {:?} → target {:?} (movement_system will handle pathfinding)",
                entity, target
            ));
            return;
        }
        None => {
            // Raycast failed (missing nodes?) → skip attack
            logger::log(&format!(
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

        logger::log(&format!(
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

        logger::log(&format!(
            "🧘 PROACTIVE: entity {:?} decides to WAIT for opening ({:.2}s)",
            entity, wait_duration
        ));
    }
}
