//! Melee combat components.
//!
//! State tracking for melee attacks, parries, and stagger.

use bevy::prelude::*;

// ============================================================================
// Attack State Component
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
// Parry State Component
// ============================================================================

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

// ============================================================================
// Stagger State Component
// ============================================================================

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

// ============================================================================
// Parry Delay Timer Component
// ============================================================================

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

// ============================================================================
// Attack Type Enum
// ============================================================================

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
