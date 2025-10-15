//! Melee combat system: attack phases, hitbox collision, defensive mechanics.
//!
//! # Architecture
//!
//! **ECS (Strategic Layer):**
//! - `MeleeAttackState`: track attack phases (windup → active → recovery)
//! - `MeleeAttackIntent`: AI wants to attack (strategic decision)
//! - `MeleeAttackStarted`: attack approved by Godot (tactical validation)
//!
//! **Godot (Tactical Layer):**
//! - `MeleeHit`: hitbox collision detected (Godot → ECS)
//! - Area3D hitbox: collision detection, animation-driven
//!
//! # Attack Flow
//!
//! ```text
//! AI decision → MeleeAttackIntent (ECS)
//!   ↓
//! Godot validates distance/LOS → MeleeAttackStarted (ECS)
//!   ↓
//! ECS adds MeleeAttackState (phase = Windup)
//!   ↓
//! Godot triggers animation + enables hitbox
//!   ↓
//! Hitbox collision → MeleeHit (Godot → ECS)
//!   ↓
//! ECS processes damage → DamageDealt event
//! ```

use bevy::prelude::*;

// ============================================================================
// Components
// ============================================================================

/// Tracks melee attack phases (windup → active → recovery).
///
/// Added to actor when they start a melee attack.
/// Removed when attack completes (returns to Idle phase).
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct MeleeAttackState {
    /// Current attack phase
    pub phase: AttackPhase,
    /// Time remaining in current phase (seconds)
    pub phase_timer: f32,
    /// Entity being attacked
    pub target: Entity,
    /// Has already hit target this attack? (prevents multiple hits)
    pub has_hit_target: bool,
}

impl MeleeAttackState {
    /// Create new attack state in Windup phase.
    pub fn new_windup(target: Entity, windup_duration: f32) -> Self {
        Self {
            phase: AttackPhase::Windup {
                duration: windup_duration,
            },
            phase_timer: windup_duration,
            target,
            has_hit_target: false,
        }
    }

    /// Check if attack is in Active phase (any sub-phase).
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            AttackPhase::ActiveParryWindow { .. } | AttackPhase::ActiveHitbox { .. }
        )
    }

    /// Check if attack is in ActiveParryWindow sub-phase.
    pub fn is_parry_window(&self) -> bool {
        matches!(self.phase, AttackPhase::ActiveParryWindow { .. })
    }

    /// Check if attack is in ActiveHitbox sub-phase (hitbox enabled).
    pub fn is_hitbox_active(&self) -> bool {
        matches!(self.phase, AttackPhase::ActiveHitbox { .. })
    }

    /// Check if attack is in Windup phase (telegraphed to enemy).
    pub fn is_windup(&self) -> bool {
        matches!(self.phase, AttackPhase::Windup { .. })
    }

    /// Check if attack is in Recovery phase (vulnerable).
    pub fn is_recovery(&self) -> bool {
        matches!(self.phase, AttackPhase::Recovery { .. })
    }

    /// Advance to next attack phase.
    ///
    /// # Phase transitions
    ///
    /// - Windup → ActiveParryWindow
    /// - ActiveParryWindow → ActiveHitbox
    /// - ActiveHitbox → Recovery
    /// - Recovery → Idle (returns None, state should be removed)
    pub fn advance_phase(&mut self) -> Option<AttackPhase> {
        match self.phase {
            AttackPhase::Windup { .. } => {
                // Transition: Windup → ActiveParryWindow
                // Duration will be set by caller (from WeaponStats)
                self.phase = AttackPhase::ActiveParryWindow { duration: 0.0 };
                Some(self.phase.clone())
            }
            AttackPhase::ActiveParryWindow { .. } => {
                // Transition: ActiveParryWindow → ActiveHitbox
                self.phase = AttackPhase::ActiveHitbox { duration: 0.0 };
                Some(self.phase.clone())
            }
            AttackPhase::ActiveHitbox { .. } => {
                // Transition: ActiveHitbox → Recovery
                self.phase = AttackPhase::Recovery { duration: 0.0 };
                Some(self.phase.clone())
            }
            AttackPhase::Recovery { .. } => {
                // Transition: Recovery → Idle (attack complete)
                self.phase = AttackPhase::Idle;
                None
            }
            AttackPhase::Idle => None,
        }
    }
}

/// Parry state component (defensive action).
///
/// Added to defender when they attempt to parry an incoming attack.
/// Parry succeeds if defender's Windup phase ends EXACTLY when attacker is in ActiveParryWindow.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct ParryState {
    /// Current parry phase (Windup → Recovery)
    pub phase: ParryPhase,

    /// Time remaining in current phase (seconds)
    pub phase_timer: f32,

    /// Entity being parried (attacker)
    pub attacker: Entity,
}

/// Parry phases (two-phase system: wind-up → recovery).
///
/// **Critical timing check happens at Windup → Recovery transition:**
/// If attacker.phase == ActiveParryWindow at this moment → PARRY SUCCESS.
#[derive(Clone, Debug, Reflect)]
pub enum ParryPhase {
    /// Windup phase (melee_parry animation, 0.1s)
    Windup { duration: f32 },

    /// Recovery phase (melee_parry_recover animation, 0.1s)
    Recovery { duration: f32 },
}

impl ParryState {
    /// Create new parry state with given attacker.
    pub fn new(attacker: Entity, windup_duration: f32) -> Self {
        Self {
            phase: ParryPhase::Windup { duration: windup_duration },
            phase_timer: windup_duration,
            attacker,
        }
    }
}

/// Stagger state component (stunned after being parried).
///
/// Added to attacker when their attack was parried.
/// Prevents all actions during stagger duration.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct StaggerState {
    /// Time remaining in stagger (seconds)
    pub timer: f32,

    /// Entity who parried us (for counter attack window)
    pub parried_by: Entity,
}

impl StaggerState {
    /// Create new stagger state.
    pub fn new(duration: f32, parried_by: Entity) -> Self {
        Self {
            timer: duration,
            parried_by,
        }
    }

    /// Check if still staggered.
    pub fn is_staggered(&self) -> bool {
        self.timer > 0.0
    }
}

/// Parry delay timer component (AI reaction timing).
///
/// Added to defender when AI decides to parry but needs to delay the action.
/// When timer expires → generates ParryIntent event.
#[derive(Component, Clone, Debug)]
pub struct ParryDelayTimer {
    /// Time remaining until parry starts (seconds)
    pub timer: f32,
    /// Entity being parried (attacker)
    pub attacker: Entity,
    /// Expected windup duration of incoming attack
    pub expected_windup_duration: f32,
}

impl ParryDelayTimer {
    /// Create new parry delay timer.
    pub fn new(delay: f32, attacker: Entity, expected_windup_duration: f32) -> Self {
        Self {
            timer: delay,
            attacker,
            expected_windup_duration,
        }
    }
}

/// Attack phases for melee combat.
///
/// # Phases
///
/// 1. **Windup**: Telegraphs attack to enemy (visible to AI)
/// 2. **ActiveParryWindow**: Parry window (hitbox OFF, can be parried)
/// 3. **ActiveHitbox**: Damage window (hitbox ON, deals damage)
/// 4. **Recovery**: Vulnerable state, cannot attack/block
/// 5. **Idle**: Not attacking (state removed from entity)
///
/// # Parry Window System
///
/// Active phase is split into two sub-phases for parry mechanics:
/// - **ActiveParryWindow** (20-30% of swing): Enemy can parry, hitbox disabled
/// - **ActiveHitbox** (30-100% of swing): Normal damage phase, hitbox enabled
///
/// This allows defenders to parry attacks during the early frames of the swing,
/// while still having a damage window if parry is missed.
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum AttackPhase {
    /// Not attacking (default state, component removed)
    Idle,
    /// Windup/telegraph phase (enemy can react)
    Windup { duration: f32 },

    /// Active phase: Parry window (hitbox OFF, can be parried)
    ///
    /// First sub-phase of attack (20-30% of total active duration).
    /// Defender can parry during this window.
    ActiveParryWindow { duration: f32 },

    /// Active phase: Hitbox enabled (deals damage)
    ///
    /// Second sub-phase of attack (30-100% of total active duration).
    /// Normal damage dealing phase.
    ActiveHitbox { duration: f32 },

    /// Recovery phase (vulnerable)
    Recovery { duration: f32 },
}

// ============================================================================
// Events
// ============================================================================

/// AI wants to perform a melee attack (ECS strategic decision).
///
/// Generated by `ai_melee_attack_intent` system when:
/// - AI is in Combat state
/// - Weapon is melee type
/// - Target is within attack radius (strategic estimate)
/// - Cooldown is ready
///
/// Processed by `process_melee_attack_intents_main_thread` (Godot tactical validation).
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackIntent {
    /// Entity initiating attack
    pub attacker: Entity,
    /// Target entity
    pub target: Entity,
    /// Attack type (Normal/Heavy/Quick)
    pub attack_type: MeleeAttackType,
}

/// Type of melee attack.
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum MeleeAttackType {
    /// Normal attack (default)
    Normal,
    /// Heavy attack (slow, high damage) - TODO: future
    Heavy,
    /// Quick attack (fast, low damage) - TODO: future
    Quick,
}

/// Melee attack has been approved and started (Godot tactical validation passed).
///
/// Generated by `process_melee_attack_intents_main_thread` after:
/// - Distance check (Godot Transform)
/// - Line of sight check (optional)
///
/// Processed by `start_melee_attacks` system (ECS):
/// - Adds `MeleeAttackState` component (phase = Windup)
/// - Starts weapon cooldown
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackStarted {
    /// Entity performing attack
    pub attacker: Entity,
    /// Target entity
    pub target: Entity,
    /// Attack type
    pub attack_type: MeleeAttackType,
    /// Windup phase duration (seconds)
    pub windup_duration: f32,
    /// Active phase duration (seconds)
    pub attack_duration: f32,
    /// Recovery phase duration (seconds)
    pub recovery_duration: f32,
}

/// Melee hitbox collision detected (Godot → ECS).
///
/// Generated by Godot when weapon hitbox (Area3D) collides with target.
/// Queued in `MELEE_HIT_QUEUE`, processed by `process_melee_hits` system.
///
/// Results in `DamageDealt` event if not blocked/parried.
#[derive(Event, Clone, Debug)]
pub struct MeleeHit {
    /// Entity that hit
    pub attacker: Entity,
    /// Entity that was hit
    pub target: Entity,
    /// Base damage (before modifiers)
    pub damage: u32,
    /// Was attack blocked? (damage reduction 70%)
    pub was_blocked: bool,
    /// Was attack parried? (damage negated 100%, attacker staggered)
    pub was_parried: bool,
}

/// Parry attempt initiated (player/AI wants to parry).
///
/// Generated by AI or player input system.
/// Processed by `start_parry` system to add ParryState component.
#[derive(Event, Clone, Debug)]
pub struct ParryIntent {
    /// Entity attempting to parry
    pub defender: Entity,
    /// Entity being parried (attacker)
    pub attacker: Entity,
    /// Expected windup duration of incoming attack (for timing calculation)
    pub expected_windup_duration: f32,
}

/// Parry successfully blocked an attack.
///
/// Generated by `detect_parry_success` system when:
/// - Attacker in ActiveParryWindow phase
/// - Defender has active ParryState
///
/// Results in:
/// - Attacker gets StaggerState
/// - Attack cancelled (skips ActiveHitbox phase)
/// - Defender gets counter attack window (TODO)
#[derive(Event, Clone, Debug)]
pub struct ParrySuccess {
    /// Entity that attacked
    pub attacker: Entity,
    /// Entity that parried
    pub defender: Entity,
}


// ============================================================================
// ECS Systems (Strategic Layer)
// ============================================================================

use crate::ai::AIState;
use crate::components::{Health, Stamina};
use crate::combat::{DamageDealt, WeaponStats};

/// System: AI generates melee attack intents (ECS strategic decision).
///
/// Runs when:
/// - AI is in Combat state
/// - Weapon is melee type
/// - Target within attack radius (strategic estimate)
/// - Weapon cooldown ready
/// - Has stamina for attack
///
/// Generates `MeleeAttackIntent` event for Godot tactical validation.
pub fn ai_melee_attack_intent(
    ai_query: Query<(Entity, &AIState, &WeaponStats, &Stamina), (Without<ParryState>, Without<ParryDelayTimer>, Without<StaggerState>, Without<MeleeAttackState>)>,
    mut intent_events: EventWriter<MeleeAttackIntent>,
) {
    for (attacker, ai_state, weapon, stamina) in ai_query.iter() {
        // Only in Combat state
        let AIState::Combat { target } = ai_state else {
            continue;
        };

        // Only melee weapons
        if !weapon.is_melee() {
            continue;
        }

        // Check weapon cooldown
        if !weapon.can_attack() {
            crate::log(&format!("⏳ {:?} melee intent: weapon on cooldown ({:.2}s)", attacker, weapon.cooldown_timer));
            continue;
        }

        // Check stamina (attack cost 30)
        const ATTACK_COST: f32 = 30.0;
        if stamina.current < ATTACK_COST {
            crate::log(&format!("💨 {:?} melee intent: not enough stamina ({:.0}/{:.0})", attacker, stamina.current, ATTACK_COST));
            continue;
        }

        // ✅ Generate intent (distance check в Godot tactical validation!)
        // ECS стратегическое решение: AI хочет атаковать
        // Godot тактическая валидация: проверит distance/LOS по актуальным Transform
        intent_events.write(MeleeAttackIntent {
            attacker,
            target: *target,
            attack_type: MeleeAttackType::Normal,
        });

        crate::log(&format!(
            "🗡️ ECS: Melee attack intent generated (attacker: {:?}, target: {:?})",
            attacker, target
        ));
    }
}

/// System: Start melee attacks (process MeleeAttackStarted events).
///
/// When Godot approves attack (tactical validation passed):
/// - Adds `MeleeAttackState` component (phase = Windup)
/// - Starts weapon cooldown
/// - Consumes stamina
/// - Generates `CombatAIEvent::EnemyAttackTelegraphed` for defender AI
pub fn start_melee_attacks(
    mut started_events: EventReader<MeleeAttackStarted>,
    mut commands: Commands,
    mut weapons: Query<&mut WeaponStats>,
    mut staminas: Query<&mut Stamina>,
    mut telegraph_events: EventWriter<crate::ai::CombatAIEvent>,
    ai_states: Query<&AIState>,
) {
    for event in started_events.read() {
        // Add MeleeAttackState (phase = Windup)
        commands.entity(event.attacker).insert(
            MeleeAttackState::new_windup(event.target, event.windup_duration)
        );

        // Start weapon cooldown
        if let Ok(mut weapon) = weapons.get_mut(event.attacker) {
            weapon.start_cooldown();
        }

        // Consume stamina (attack cost)
        const ATTACK_COST: f32 = 30.0;
        if let Ok(mut stamina) = staminas.get_mut(event.attacker) {
            stamina.consume(ATTACK_COST);
        }

        // Generate EnemyAttackTelegraphed event (ONLY if defender in Combat state)
        if let Ok(defender_ai) = ai_states.get(event.target) {
            if matches!(defender_ai, AIState::Combat { .. }) {
                telegraph_events.write(crate::ai::CombatAIEvent::EnemyAttackTelegraphed {
                    attacker: event.attacker,
                    target: event.target,
                    attack_type: crate::combat::AttackType::Melee,
                    windup_remaining: event.windup_duration,
                });

                crate::log(&format!(
                    "📣 ECS: EnemyAttackTelegraphed (attacker: {:?} → defender: {:?}, windup: {:.2}s)",
                    event.attacker, event.target, event.windup_duration
                ));
            }
        }

        crate::log(&format!(
            "⚔️ ECS: Melee attack started (attacker: {:?}, target: {:?}, windup: {:.2}s)",
            event.attacker, event.target, event.windup_duration
        ));
    }
}

/// System: Update melee attack phases (windup → active → recovery → idle).
///
/// Advances attack phases based on timers.
/// When phase = Idle → removes MeleeAttackState component.
pub fn update_melee_attack_phases(
    mut query: Query<(Entity, &mut MeleeAttackState)>,
    weapons: Query<&WeaponStats>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
) {
    let delta = time.delta_secs();

    for (entity, mut attack_state) in query.iter_mut() {
        // Decrease phase timer
        attack_state.phase_timer -= delta;

        // Phase transition when timer expires
        if attack_state.phase_timer <= 0.0 {
            let Some(new_phase) = attack_state.advance_phase() else {
                // Attack complete (Idle) → remove component
                commands.entity(entity).remove::<MeleeAttackState>();
                crate::log(&format!("✅ ECS: Melee attack completed (entity: {:?})", entity));
                continue;
            };

            // Get weapon stats for phase durations
            let Ok(weapon) = weapons.get(entity) else {
                continue;
            };

            // Set new phase timer based on phase type
            match new_phase {
                AttackPhase::ActiveParryWindow { .. } => {
                    // Parry window: weapon.parry_window duration
                    attack_state.phase = AttackPhase::ActiveParryWindow {
                        duration: weapon.parry_window,
                    };
                    attack_state.phase_timer = weapon.parry_window;
                    crate::log(&format!(
                        "⚔️ ECS: Windup → ActiveParryWindow ({:.3}s) (entity: {:?})",
                        weapon.parry_window, entity
                    ));
                }
                AttackPhase::ActiveHitbox { .. } => {
                    // Hitbox window: attack_duration - parry_window
                    let hitbox_duration = weapon.attack_duration - weapon.parry_window;
                    attack_state.phase = AttackPhase::ActiveHitbox {
                        duration: hitbox_duration,
                    };
                    attack_state.phase_timer = hitbox_duration;
                    crate::log(&format!(
                        "💥 ECS: ActiveParryWindow → ActiveHitbox ({:.3}s) (entity: {:?})",
                        hitbox_duration, entity
                    ));
                }
                AttackPhase::Recovery { .. } => {
                    attack_state.phase = AttackPhase::Recovery {
                        duration: weapon.recovery_duration,
                    };
                    attack_state.phase_timer = weapon.recovery_duration;
                    crate::log(&format!("🛡️ ECS: ActiveHitbox → Recovery (entity: {:?})", entity));
                }
                _ => {}
            }
        }
    }
}

/// System: Process melee hits (Godot → ECS damage application).
///
/// Reads `MeleeHit` events, applies damage with modifiers:
/// - Blocked: 70% damage reduction
/// - Parried: 100% damage negation + stagger attacker
/// - Normal: full damage
///
/// Generates `DamageDealt` events.
pub fn process_melee_hits(
    mut melee_hit_events: EventReader<MeleeHit>,
    mut damage_dealt_events: EventWriter<DamageDealt>,
    mut healths: Query<&mut Health>,
    _weapons: Query<&WeaponStats>,
) {
    for hit in melee_hit_events.read() {
        // Skip self-hits
        if hit.attacker == hit.target {
            continue;
        }

        // Calculate damage with modifiers
        let mut final_damage = hit.damage;

        if hit.was_parried {
            // Parried: 100% negation
            final_damage = 0;
            crate::log(&format!(
                "🛡️ Melee hit PARRIED (attacker: {:?}, target: {:?})",
                hit.attacker, hit.target
            ));

            // Stagger attacker (increase cooldown by 0.5s)
            // TODO: Implement when parry system is ready

        } else if hit.was_blocked {
            // Blocked: 70% reduction
            final_damage = (final_damage as f32 * 0.3) as u32;
            crate::log(&format!(
                "🛡️ Melee hit BLOCKED (attacker: {:?}, target: {:?}, reduced damage: {})",
                hit.attacker, hit.target, final_damage
            ));
        }

        // Apply damage
        if final_damage > 0 {
            if let Ok(mut health) = healths.get_mut(hit.target) {
                health.take_damage(final_damage);

                // Generate DamageDealt event
                damage_dealt_events.write(DamageDealt {
                    attacker: hit.attacker,
                    target: hit.target,
                    damage: final_damage,
                    source: crate::combat::DamageSource::Melee,
                });

                crate::log(&format!(
                    "💥 Melee damage dealt (attacker: {:?}, target: {:?}, damage: {}, health: {} → {})",
                    hit.attacker, hit.target, final_damage,
                    health.current + final_damage, health.current
                ));
            }
        }
    }
}

// ============================================================================
// Parry Systems
// ============================================================================

/// System: Start parry (process ParryIntent events).
///
/// Adds ParryState component to defender.
/// Parry timing is critical: defender must finish Windup phase exactly when attacker is in ActiveParryWindow.
pub fn start_parry(
    mut intent_events: EventReader<ParryIntent>,
    mut commands: Commands,
    weapons: Query<&WeaponStats>,
) {
    for intent in intent_events.read() {
        // Get weapon stats for parry check
        let Ok(weapon) = weapons.get(intent.defender) else {
            continue;
        };

        // Check if weapon can parry
        if !weapon.can_parry() {
            crate::log(&format!(
                "❌ ECS: {:?} cannot parry (weapon doesn't support it)",
                intent.defender
            ));
            continue;
        }

        // Parry windup duration (melee_parry animation length)
        // Fixed duration: 0.1s
        let parry_windup = 0.1;

        // Add ParryState component
        commands
            .entity(intent.defender)
            .insert(ParryState::new(intent.attacker, parry_windup));

        crate::log(&format!(
            "🛡️ ECS: Parry started (defender: {:?}, attacker: {:?}, windup: {:.2}s)",
            intent.defender, intent.attacker, parry_windup
        ));
    }
}

/// System: Update parry states and check for parry success at critical moment.
///
/// **Critical timing check:**
/// When ParryState transitions from Windup → Recovery, checks if attacker is in ActiveParryWindow.
/// If yes → PARRY SUCCESS (stagger attacker, cancel attack).
/// If no → parry failed, defender enters recovery vulnerable state.
pub fn update_parry_states(
    mut query: Query<(Entity, &mut ParryState)>,
    attacks: Query<&MeleeAttackState>,
    weapons: Query<&WeaponStats>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
) {
    let delta = time.delta_secs();

    for (defender, mut parry_state) in query.iter_mut() {
        parry_state.phase_timer -= delta;

        // Check if phase transition happens
        if parry_state.phase_timer <= 0.0 {
            match &parry_state.phase {
                ParryPhase::Windup { .. } => {
                    // 🎯 CRITICAL MOMENT: Parry windup ended!
                    // Check if attacker is in ActiveParryWindow phase

                    let mut parry_success = false;

                    if let Ok(attack_state) = attacks.get(parry_state.attacker) {
                        if matches!(attack_state.phase, AttackPhase::ActiveParryWindow { .. }) {
                            // ✅ PARRY SUCCESS!

                            let Ok(weapon) = weapons.get(parry_state.attacker) else {
                                continue;
                            };

                            // Add StaggerState to attacker + remove MeleeAttackState
                            commands.entity(parry_state.attacker)
                                .insert(StaggerState::new(weapon.stagger_duration, defender))
                                .remove::<MeleeAttackState>();

                            crate::log(&format!(
                                "💥 ECS: PARRY SUCCESS! (defender: {:?}, attacker: {:?} staggered)",
                                defender, parry_state.attacker
                            ));

                            parry_success = true;

                            // Transition to Recovery (play parry_recover animation)
                            let recovery_duration = 0.1;
                            parry_state.phase = ParryPhase::Recovery { duration: recovery_duration };
                            parry_state.phase_timer = recovery_duration;
                        } else {
                            // ❌ PARRY FAIL (wrong timing) → still transition to Recovery
                            crate::log(&format!(
                                "❌ ECS: PARRY FAIL - wrong timing (defender: {:?}, attacker phase: {:?})",
                                defender, attack_state.phase
                            ));

                            let recovery_duration = 0.1;
                            parry_state.phase = ParryPhase::Recovery { duration: recovery_duration };
                            parry_state.phase_timer = recovery_duration;
                        }
                    } else {
                        // Attacker no longer exists or not attacking → still transition to Recovery
                        crate::log(&format!(
                            "❌ ECS: PARRY FAIL - attacker {:?} not found or not attacking",
                            parry_state.attacker
                        ));

                        let recovery_duration = 0.1;
                        parry_state.phase = ParryPhase::Recovery { duration: recovery_duration };
                        parry_state.phase_timer = recovery_duration;
                    }
                }

                ParryPhase::Recovery { .. } => {
                    // Recovery ended → remove ParryState
                    commands.entity(defender).remove::<ParryState>();
                    crate::log(&format!("⏱️ ECS: Parry recovery complete (entity: {:?})", defender));
                }
            }
        }
    }
}


/// System: Update stagger states (tick timers, remove expired).
pub fn update_stagger_states(
    mut query: Query<(Entity, &mut StaggerState)>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
) {
    let delta = time.delta_secs();

    for (entity, mut stagger) in query.iter_mut() {
        // Decrease timer
        stagger.timer -= delta;

        // Remove when stagger expires
        if stagger.timer <= 0.0 {
            commands.entity(entity).remove::<StaggerState>();
            crate::log(&format!(
                "✅ ECS: Stagger ended (entity: {:?})",
                entity
            ));
        }
    }
}

/// System: Process parry delay timers (AI reaction timing).
///
/// Ticks ParryDelayTimer components and generates ParryIntent when timer expires.
/// This creates realistic AI reaction timing for parry decisions.
pub fn process_parry_delay_timers(
    mut query: Query<(Entity, &mut ParryDelayTimer)>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
    mut parry_intent_events: EventWriter<ParryIntent>,
) {
    let delta = time.delta_secs();

    for (defender, mut delay_timer) in query.iter_mut() {
        delay_timer.timer -= delta;

        // Timer expired → generate ParryIntent
        if delay_timer.timer <= 0.0 {
            parry_intent_events.write(ParryIntent {
                defender,
                attacker: delay_timer.attacker,
                expected_windup_duration: delay_timer.expected_windup_duration,
            });

            // Remove timer component
            commands.entity(defender).remove::<ParryDelayTimer>();

            crate::log(&format!(
                "⏰ ECS: Parry delay expired → ParryIntent (defender: {:?}, attacker: {:?})",
                defender, delay_timer.attacker
            ));
        }
    }
}
