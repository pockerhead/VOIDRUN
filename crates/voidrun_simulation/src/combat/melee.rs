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
///
/// **CHANGED:** No target field (area-based, multi-target support).
/// `hit_entities` tracks all entities hit this attack (prevents double-hits).
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct MeleeAttackState {
    /// Current attack phase
    pub phase: AttackPhase,
    /// Time remaining in current phase (seconds)
    pub phase_timer: f32,
    /// Entities already hit during this attack (prevents multiple hits on same target)
    pub hit_entities: Vec<Entity>,
}

impl MeleeAttackState {
    /// Create new attack state in Windup phase.
    pub fn new_windup(windup_duration: f32) -> Self {
        Self {
            phase: AttackPhase::Windup {
                duration: windup_duration,
            },
            phase_timer: windup_duration,
            hit_entities: Vec::new(),
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
///
/// **Note:** `attacker` can be None for idle/defensive parry (player practicing or preemptive defense).
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct ParryState {
    /// Current parry phase (Windup → Recovery)
    pub phase: ParryPhase,

    /// Time remaining in current phase (seconds)
    pub phase_timer: f32,

    /// Entity being parried (attacker).
    /// - `Some(entity)`: Targeted parry (timing check enabled)
    /// - `None`: Idle parry (animation only, no timing check)
    pub attacker: Option<Entity>,
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
    /// Create new parry state with optional attacker.
    ///
    /// # Arguments
    /// - `attacker`: Some(entity) for targeted parry, None for idle/defensive parry
    /// - `windup_duration`: Animation duration (fixed 0.1s)
    pub fn new(attacker: Option<Entity>, windup_duration: f32) -> Self {
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
///
/// **CHANGED:** No target field (area-based detection, hitbox determines targets).
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackIntent {
    /// Entity initiating attack
    pub attacker: Entity,
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
///
/// **CHANGED:** No target field (area-based detection, hitbox determines targets).
#[derive(Event, Clone, Debug)]
pub struct MeleeAttackStarted {
    /// Entity performing attack
    pub attacker: Entity,
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
    /// Точка попадания (target body center, для VFX)
    pub impact_point: Vec3,
    /// Нормаль поверхности (attacker→target direction, для VFX)
    pub impact_normal: Vec3,
}

/// Parry attempt initiated (player/AI wants to parry).
///
/// Generated by AI or player input system.
/// Processed by `start_parry` system to add ParryState component.
///
/// **Note:** `attacker` can be None for idle/defensive parry (player practicing).
#[derive(Event, Clone, Debug)]
pub struct ParryIntent {
    /// Entity attempting to parry
    pub defender: Entity,
    /// Entity being parried (attacker).
    /// - `Some(entity)`: Targeted parry (AI or player with visible enemy)
    /// - `None`: Idle parry (player training/preemptive defense)
    pub attacker: Option<Entity>,
    /// Expected windup duration of incoming attack (unused for idle parry)
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

use crate::components::{Health, Stamina};
use crate::combat::{DamageDealt, WeaponStats};

// REMOVED: ai_melee_attack_intent
// Replaced by unified ai_combat_decision_main_thread system (see ai_combat_decision.rs)
// That system handles both attack AND parry decisions to prevent race conditions.

/// System: Start melee attacks (process MeleeAttackStarted events).
///
/// When Godot approves attack (tactical validation passed):
/// - Adds `MeleeAttackState` component (phase = Windup)
/// - Starts weapon cooldown
/// - Consumes stamina
///
/// **CHANGED:** No longer generates telegraph events (handled by `detect_melee_windups_main_thread`).
pub fn start_melee_attacks(
    mut started_events: EventReader<MeleeAttackStarted>,
    mut commands: Commands,
    mut weapons: Query<&mut WeaponStats>,
    mut staminas: Query<&mut Stamina>,
) {
    for event in started_events.read() {
        // Add MeleeAttackState (phase = Windup)
        commands.entity(event.attacker).insert(
            MeleeAttackState::new_windup(event.windup_duration)
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

        crate::log(&format!(
            "⚔️ ECS: Melee attack started (attacker: {:?}, windup: {:.2}s)",
            event.attacker, event.windup_duration
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
/// - Normal: full damage (bypasses shield, slow kinetic)
///
/// Generates `DamageDealt` events with impact data.
pub fn process_melee_hits(
    mut melee_hit_events: EventReader<MeleeHit>,
    mut damage_dealt_events: EventWriter<DamageDealt>,
    mut healths: Query<(&mut Health, Option<&mut crate::components::EnergyShield>)>,
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

        // Apply damage (melee bypasses shield)
        if final_damage > 0 {
            let Ok((mut health, mut shield_opt)) = healths.get_mut(hit.target) else {
                continue;
            };

            let applied = crate::combat::apply_damage_with_shield(
                &mut health,
                shield_opt.as_deref_mut(),
                final_damage,
                crate::combat::DamageSource::Melee,
            );

            // Generate DamageDealt event with impact data
            damage_dealt_events.write(DamageDealt {
                attacker: hit.attacker,
                target: hit.target,
                damage: final_damage,
                source: crate::combat::DamageSource::Melee,
                applied_damage: applied,
                impact_point: hit.impact_point,
                impact_normal: hit.impact_normal,
            });

            crate::log(&format!(
                "💥 Melee damage dealt (attacker: {:?}, target: {:?}, damage: {}, applied: {:?}, HP: {})",
                hit.attacker, hit.target, final_damage, applied, health.current
            ));
        }
    }
}

// ============================================================================
// Parry Systems
// ============================================================================

/// System: Start parry (process ParryIntent events).
///
/// Adds ParryState component to defender.
/// Supports both targeted parry (with attacker) and idle parry (no attacker).
///
/// **Targeted parry:** Timing check enabled (must match attacker's ActiveParryWindow).
/// **Idle parry:** Animation only (no timing check, always "fails" but plays animation).
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

        // Add ParryState component (attacker can be None for idle parry)
        commands
            .entity(intent.defender)
            .insert(ParryState::new(intent.attacker, parry_windup));

        // Log based on parry type
        if let Some(attacker) = intent.attacker {
            crate::log(&format!(
                "🛡️ ECS: Targeted parry started (defender: {:?}, attacker: {:?}, windup: {:.2}s)",
                intent.defender, attacker, parry_windup
            ));
        } else {
            crate::log(&format!(
                "🛡️ ECS: Idle parry started (defender: {:?}, windup: {:.2}s)",
                intent.defender, parry_windup
            ));
        }
    }
}

/// System: Update parry states and check for parry success at critical moment.
///
/// **Critical timing check:**
/// When ParryState transitions from Windup → Recovery, checks if attacker is in ActiveParryWindow.
/// If yes → PARRY SUCCESS (stagger attacker, cancel attack).
/// If no → parry failed, defender enters recovery vulnerable state.
///
/// **Idle parry:** If attacker is None, plays animation only (no timing check).
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

                    // Helper: transition to recovery phase (DRY)
                    let transition_to_recovery = |state: &mut ParryState| {
                        let recovery_duration = 0.1;
                        state.phase = ParryPhase::Recovery { duration: recovery_duration };
                        state.phase_timer = recovery_duration;
                    };

                    // Get attacker entity (or None for idle parry)
                    let Some(attacker_entity) = parry_state.attacker else {
                        // Idle parry: просто проигрываем анимацию (no timing check)
                        crate::log(&format!(
                            "🛡️ ECS: Idle parry completed (defender: {:?})",
                            defender
                        ));

                        transition_to_recovery(&mut parry_state);
                        continue;
                    };

                    // Targeted parry: get attacker's attack state
                    let Ok(attack_state) = attacks.get(attacker_entity) else {
                        crate::log(&format!(
                            "❌ ECS: PARRY FAIL - attacker {:?} not found or not attacking",
                            attacker_entity
                        ));

                        transition_to_recovery(&mut parry_state);
                        continue;
                    };

                    // Check timing: attacker must be in ActiveParryWindow
                    if matches!(attack_state.phase, AttackPhase::ActiveParryWindow { .. }) {
                        // ✅ PARRY SUCCESS!
                        let Ok(weapon) = weapons.get(attacker_entity) else {
                            continue;
                        };

                        // Stagger attacker + remove attack
                        commands.entity(attacker_entity)
                            .insert(StaggerState::new(weapon.stagger_duration, defender))
                            .remove::<MeleeAttackState>();

                        crate::log(&format!(
                            "💥 ECS: PARRY SUCCESS! (defender: {:?}, attacker: {:?} staggered)",
                            defender, attacker_entity
                        ));
                    } else {
                        // ❌ PARRY FAIL - wrong timing
                        crate::log(&format!(
                            "❌ ECS: PARRY FAIL - wrong timing (defender: {:?}, attacker phase: {:?})",
                            defender, attack_state.phase
                        ));
                    }

                    // Transition to recovery (both success and fail paths)
                    transition_to_recovery(&mut parry_state);
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

        // Timer expired → generate ParryIntent (AI always uses Some(attacker))
        if delay_timer.timer <= 0.0 {
            parry_intent_events.write(ParryIntent {
                defender,
                attacker: Some(delay_timer.attacker),  // AI targeted parry
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
