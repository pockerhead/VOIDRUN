//! Melee combat systems (strategic layer logic).

use bevy::prelude::*;
use crate::components::{Health, Stamina};
use crate::combat::{
    DamageDealt, MeleeAttackStarted, MeleeHit, ParryIntent,
    MeleeAttackState, AttackPhase, ParryState, ParryPhase, StaggerState, ParryDelayTimer,
    WeaponStats,
};

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

        crate::logger::log(&format!(
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
                crate::logger::log(&format!("✅ ECS: Melee attack completed (entity: {:?})", entity));
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
                    crate::logger::log(&format!(
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
                    crate::logger::log(&format!(
                        "💥 ECS: ActiveParryWindow → ActiveHitbox ({:.3}s) (entity: {:?})",
                        hitbox_duration, entity
                    ));
                }
                AttackPhase::Recovery { .. } => {
                    attack_state.phase = AttackPhase::Recovery {
                        duration: weapon.recovery_duration,
                    };
                    attack_state.phase_timer = weapon.recovery_duration;
                    crate::logger::log(&format!("🛡️ ECS: ActiveHitbox → Recovery (entity: {:?})", entity));
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
            crate::logger::log(&format!(
                "🛡️ Melee hit PARRIED (attacker: {:?}, target: {:?})",
                hit.attacker, hit.target
            ));

            // Stagger attacker (increase cooldown by 0.5s)
            // TODO: Implement when parry system is ready

        } else if hit.was_blocked {
            // Blocked: 70% reduction
            final_damage = (final_damage as f32 * 0.3) as u32;
            crate::logger::log(&format!(
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

            crate::logger::log(&format!(
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
            crate::logger::log(&format!(
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
            crate::logger::log(&format!(
                "🛡️ ECS: Targeted parry started (defender: {:?}, attacker: {:?}, windup: {:.2}s)",
                intent.defender, attacker, parry_windup
            ));
        } else {
            crate::logger::log(&format!(
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
                        crate::logger::log(&format!(
                            "🛡️ ECS: Idle parry completed (defender: {:?})",
                            defender
                        ));

                        transition_to_recovery(&mut parry_state);
                        continue;
                    };

                    // Targeted parry: get attacker's attack state
                    let Ok(attack_state) = attacks.get(attacker_entity) else {
                        crate::logger::log(&format!(
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

                        crate::logger::log(&format!(
                            "💥 ECS: PARRY SUCCESS! (defender: {:?}, attacker: {:?} staggered)",
                            defender, attacker_entity
                        ));
                    } else {
                        // ❌ PARRY FAIL - wrong timing
                        crate::logger::log(&format!(
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
                    crate::logger::log(&format!("⏱️ ECS: Parry recovery complete (entity: {:?})", defender));
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
            crate::logger::log(&format!(
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

            crate::logger::log(&format!(
                "⏰ ECS: Parry delay expired → ParryIntent (defender: {:?}, attacker: {:?})",
                defender, delay_timer.attacker
            ));
        }
    }
}
