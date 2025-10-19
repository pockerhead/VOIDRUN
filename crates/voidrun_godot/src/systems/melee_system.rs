//! Melee combat system (Godot tactical layer).
//!
//! # Architecture
//!
//! **ECS (Strategic Layer):**
//! - `MeleeAttackIntent`: AI wants to attack (strategic decision)
//! - `MeleeAttackStarted`: Attack approved by Godot (tactical validation)
//! - `MeleeHit`: Hitbox collision detected (Godot → ECS damage)
//!
//! **Godot (Tactical Layer):**
//! - Distance/LOS validation (Godot Transform)
//! - Animation trigger (windup → active → recovery)
//! - Hitbox collision detection (Area3D)
//!
//! # Flow
//!
//! ```text
//! ECS: MeleeAttackIntent
//!   ↓
//! Godot: process_melee_attack_intents_main_thread (distance check)
//!   ↓
//! ECS: MeleeAttackStarted → adds MeleeAttackState
//!   ↓
//! Godot: execute_melee_attacks_main_thread (animation + hitbox)
//!   ↓
//! Godot: Area3D collision → MeleeHit event
//!   ↓
//! ECS: process_melee_hits → DamageDealt
//! ```

use bevy::prelude::*;
use godot::prelude::*;
use voidrun_simulation::combat::{
    MeleeAttackIntent, MeleeAttackStarted, MeleeAttackState, AttackPhase,
    WeaponStats, ParryState,
};

use crate::systems::{VisualRegistry, AttachmentRegistry};

/// System: Process melee attack intents (Godot tactical validation).
///
/// Validates:
/// - Attacker has weapon
/// - Attacker not already attacking
///
/// **CHANGED:** No distance/LOS check (area-based detection, hitbox determines targets).
/// If validation passes → generates `MeleeAttackStarted` event.
pub fn process_melee_attack_intents_main_thread(
    mut intent_events: EventReader<MeleeAttackIntent>,
    weapons: Query<&WeaponStats>,
    attack_states: Query<&MeleeAttackState>,
    mut started_events: EventWriter<MeleeAttackStarted>,
) {
    for intent in intent_events.read() {
        voidrun_simulation::log(&format!("📥 Godot: Received melee intent (attacker: {:?})", intent.attacker));

        // Skip if attacker already has MeleeAttackState (attack in progress)
        if attack_states.get(intent.attacker).is_ok() {
            voidrun_simulation::log(&format!("⏸️ Godot: Attacker {:?} already attacking, ignoring intent", intent.attacker));
            continue;
        }

        // Get weapon stats for attack parameters
        let Ok(weapon) = weapons.get(intent.attacker) else {
            voidrun_simulation::log(&format!("❌ Godot: attacker {:?} has no weapon", intent.attacker));
            continue;
        };

        // Validation passed → generate MeleeAttackStarted
        started_events.write(MeleeAttackStarted {
            attacker: intent.attacker,
            attack_type: intent.attack_type.clone(),
            windup_duration: weapon.windup_duration,
            attack_duration: weapon.attack_duration,
            recovery_duration: weapon.recovery_duration,
        });

        voidrun_simulation::log(&format!(
            "⚔️ Godot: Melee attack validated (attacker: {:?})",
            intent.attacker
        ));
    }
}

/// System: Execute melee attacks (animation + hitbox control).
///
/// Listens to `MeleeAttackState` phase changes:
/// - Windup → trigger animation "attack_windup"
/// - Active → enable weapon hitbox (Area3D.monitoring = true)
/// - Recovery → disable hitbox (Area3D.monitoring = false)
/// - Idle → (no action, state removed by ECS)
///
/// Uses `Changed<MeleeAttackState>` to react only when phase changes.
///
/// **Animation speed adjustment:**
/// Dynamically adjusts AnimationPlayer speed_scale to match weapon timings.
/// This allows different weapon types to have different attack speeds without
/// creating separate animation files.
pub fn execute_melee_attacks_main_thread(
    query: Query<(Entity, &MeleeAttackState), Changed<MeleeAttackState>>,
    visuals: NonSend<VisualRegistry>,
    attachments: NonSend<AttachmentRegistry>,
    weapons: Query<&WeaponStats>,
) {
    for (entity, attack_state) in query.iter() {
        // Get attacker node
        let Some(_attacker_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Get weapon attachment (for hitbox control)
        let Some(weapon_attachment) = attachments.attachments.get(&(entity, "%RightHandAttachment".to_string())) else {
            voidrun_simulation::log(&format!(
                "⚠️ Godot: Melee attack entity {:?} has no weapon attachment",
                entity
            ));
            continue;
        };

        // Get weapon stats for animation speed calculation
        let Ok(weapon) = weapons.get(entity) else {
            continue;
        };

        // Get AnimationPlayer for triggering animations
        let attacker_node = visuals.visuals.get(&entity).unwrap();
        let anim_player = attacker_node
            .try_get_node_as::<godot::classes::AnimationPlayer>("MeleeSwingAnimationPlayer");

        if anim_player.is_none() {
            voidrun_simulation::log(&format!("⚠️ Godot: Entity {:?} has no AnimationPlayer!", entity));
        }

        // Handle phase transitions
        match &attack_state.phase {
            AttackPhase::Windup { duration } => {
                // Trigger windup animation with dynamic speed
                if let Some(mut player) = anim_player {
                    let anim_length = get_animation_length(&mut player, "melee_windup");
                    let speed_scale = anim_length / duration;

                    player.set_speed_scale(speed_scale);
                    player.play_ex().name("melee_windup").done();

                    voidrun_simulation::log(&format!(
                        "▶️ Godot: Playing 'melee_windup' (entity: {:?}, duration: {:.2}s, speed: {:.2}x)",
                        entity, duration, speed_scale
                    ));
                } else {
                    voidrun_simulation::log(&format!(
                        "❌ Godot: Cannot play windup animation - no AnimationPlayer (entity: {:?})",
                        entity
                    ));
                }
            }

            AttackPhase::ActiveParryWindow { duration } => {
                // Parry window: swing animation starts, hitbox DISABLED
                if let Some(mut player) = anim_player {
                    let anim_length = get_animation_length(&mut player, "melee_swing");

                    // Calculate total active duration (parry + hitbox)
                    let total_active = weapon.attack_duration;
                    let speed_scale = anim_length / total_active;

                    player.set_speed_scale(speed_scale);
                    player.play_ex().name("melee_swing").done();

                    voidrun_simulation::log(&format!(
                        "⚔️ Godot: Playing 'melee_swing' (ActiveParryWindow) (entity: {:?}, duration: {:.3}s, speed: {:.2}x, hitbox: OFF)",
                        entity, duration, speed_scale
                    ));
                }
                // Hitbox OFF during parry window
                enable_weapon_hitbox(&weapon_attachment, false);
            }

            AttackPhase::ActiveHitbox { duration } => {
                // Hitbox window: enable hitbox (animation continues)
                voidrun_simulation::log(&format!(
                    "💥 Godot: ActiveHitbox phase (entity: {:?}, duration: {:.3}s, hitbox: ON)",
                    entity, duration
                ));

                // Enable hitbox (animation already playing from ActiveParryWindow)
                enable_weapon_hitbox(&weapon_attachment, true);
            }

            AttackPhase::Recovery { duration } => {
                // Trigger recovery animation + disable hitbox
                if let Some(mut player) = anim_player {
                    let anim_length = get_animation_length(&mut player, "melee_recovery");
                    let speed_scale = anim_length / duration;

                    player.set_speed_scale(speed_scale);
                    player.play_ex().name("melee_recovery").done();

                    voidrun_simulation::log(&format!(
                        "🛡️ Godot: Playing 'melee_recovery' (entity: {:?}, duration: {:.2}s, speed: {:.2}x)",
                        entity, duration, speed_scale
                    ));
                }
                enable_weapon_hitbox(&weapon_attachment, false);
            }

            AttackPhase::Idle => {
                // Reset to idle pose
                if let Some(mut player) = anim_player {
                    player.set_speed_scale(1.0); // Reset speed to normal
                    player.play_ex().name("RESET").done();
                }
            }
        }
    }
}

/// System: Poll melee hitbox overlaps during Active phase
///
/// Checks Area3D.get_overlapping_bodies() every frame during Active phase.
/// Generates MeleeHit event when hitbox touches enemy body.
///
/// **Anti-spam:** Uses `hit_entities` to track all entities hit this attack.
/// **CHANGED:** Multi-target support (cleave damage), no single target restriction.
pub fn poll_melee_hitboxes_main_thread(
    mut query: Query<(Entity, &mut MeleeAttackState)>,
    visuals: NonSend<VisualRegistry>,
    attachments: NonSend<AttachmentRegistry>,
    mut melee_hit_events: EventWriter<voidrun_simulation::combat::MeleeHit>,
) {
    for (attacker, mut attack_state) in query.iter_mut() {
        // Only check during ActiveHitbox phase (NOT ActiveParryWindow!)
        let AttackPhase::ActiveHitbox { .. } = &attack_state.phase else {
            continue;
        };

        // Get weapon attachment
        let Some(weapon_attachment) = attachments.attachments.get(&(attacker, "%RightHandAttachment".to_string())) else {
            continue;
        };

        // Get hitbox
        let Some(weapon_placement) = weapon_attachment.try_get_node_as::<Node3D>("WeaponPlacement") else {
            continue;
        };
        let Some(hitbox) = weapon_placement.try_get_node_as::<godot::classes::Area3D>("Hitbox") else {
            continue;
        };

        // Poll overlapping bodies
        let overlapping = hitbox.get_overlapping_bodies();
        for i in 0..overlapping.len() {
            if let Some(body) = overlapping.get(i) {
                let instance_id = body.instance_id();

                // Reverse lookup: Godot InstanceId → ECS Entity
                if let Some(&target_entity) = visuals.node_to_entity.get(&instance_id) {
                    // Don't hit yourself
                    if target_entity == attacker {
                        continue;
                    }

                    // Skip if already hit this entity (prevent double-hits)
                    if attack_state.hit_entities.contains(&target_entity) {
                        continue;
                    }

                    // Generate MeleeHit event
                    // TODO: Calculate actual damage from weapon stats
                    melee_hit_events.write(voidrun_simulation::combat::MeleeHit {
                        attacker,
                        target: target_entity,
                        damage: 20, // TODO: Get from WeaponStats
                        was_blocked: false, // TODO: Check target block state
                        was_parried: false, // TODO: Check target parry state
                    });

                    // Track entity as hit (prevent multiple hits on same target)
                    attack_state.hit_entities.push(target_entity);

                    voidrun_simulation::log(&format!(
                        "💥 Godot: Melee hit detected! (attacker: {:?}, target: {:?})",
                        attacker, target_entity
                    ));

                    // Continue to allow cleave damage (multi-target hits)
                }
            }
        }
    }
}

/// Get animation length from AnimationPlayer.
///
/// Returns the length of the specified animation in seconds.
/// Falls back to 1.0 if animation not found.
fn get_animation_length(player: &mut Gd<godot::classes::AnimationPlayer>, anim_name: &str) -> f32 {
    // Get animation library
    let Some(lib) = player.get_animation_library("") else {
        voidrun_simulation::log(&format!("⚠️ Godot: No default animation library found"));
        return 1.0;
    };

    // Get animation from library
    let Some(anim) = lib.get_animation(anim_name) else {
        voidrun_simulation::log(&format!("⚠️ Godot: Animation '{}' not found", anim_name));
        return 1.0;
    };

    anim.get_length() as f32
}

/// Enable/disable weapon hitbox (Area3D monitoring).
///
/// Searches for "Hitbox" child node under weapon attachment.
fn enable_weapon_hitbox(weapon_node: &Gd<Node3D>, enabled: bool) {
    // Find hitbox node (assumes structure: Weapon/WeaponPlacement/Hitbox)
    let Some(weapon_placement) = weapon_node
        .try_get_node_as::<Node3D>("WeaponPlacement")
    else {
        voidrun_simulation::log("⚠️ Godot: Weapon has no WeaponPlacement node");
        return;
    };

    let Some(mut hitbox) = weapon_placement
        .try_get_node_as::<godot::classes::Area3D>("Hitbox")
    else {
        voidrun_simulation::log("⚠️ Godot: Weapon has no Hitbox node");
        return;
    };

    // Enable/disable monitoring (collision detection)
    hitbox.set_monitoring(enabled);

    if enabled {
        voidrun_simulation::log("✅ Godot: Weapon hitbox enabled");
    } else {
        voidrun_simulation::log("❌ Godot: Weapon hitbox disabled");
    }
}

/// System: Execute parry animations (two-phase: Windup → Recovery).
///
/// Listens to `Changed<ParryState>` to trigger animations for each phase:
/// - Windup: plays "melee_parry" (0.1s)
/// - Recovery: plays "melee_parry_recover" (0.1s)
pub fn execute_parry_animations_main_thread(
    query: Query<(Entity, &ParryState), Changed<ParryState>>,
    visuals: NonSend<VisualRegistry>,
) {
    use voidrun_simulation::combat::ParryPhase;

    for (entity, parry_state) in query.iter() {
        // Get defender node
        let Some(defender_node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Get DefenceAnimationPlayer (отдельный player для парирования)
        let Some(mut anim_player) = defender_node
            .try_get_node_as::<godot::classes::AnimationPlayer>("DefenceAnimationPlayer")
        else {
            voidrun_simulation::log(&format!(
                "⚠️ Godot: Entity {:?} has no DefenceAnimationPlayer for parry animation",
                entity
            ));
            continue;
        };

        // Play animation based on current phase
        match &parry_state.phase {
            ParryPhase::Windup { duration } => {
                // Play melee_parry animation with speed adjusted to windup duration
                let anim_length = get_animation_length(&mut anim_player, "melee_parry");
                let speed_scale = anim_length / duration;

                anim_player.set_speed_scale(speed_scale);
                anim_player.play_ex().name("melee_parry").done();

                voidrun_simulation::log(&format!(
                    "🛡️ Godot: Playing 'melee_parry' (Windup) (entity: {:?}, duration: {:.3}s, speed: {:.2}x)",
                    entity, duration, speed_scale
                ));
            }

            ParryPhase::Recovery { duration } => {
                // Play melee_parry_recover animation
                let anim_length = get_animation_length(&mut anim_player, "melee_parry_recover");
                let speed_scale = anim_length / duration;

                anim_player.set_speed_scale(speed_scale);
                anim_player.play_ex().name("melee_parry_recover").done();

                voidrun_simulation::log(&format!(
                    "🛡️ Godot: Playing 'melee_parry_recover' (Recovery) (entity: {:?}, duration: {:.3}s, speed: {:.2}x)",
                    entity, duration, speed_scale
                ));
            }
        }
    }
}

/// System: Execute stagger animations when StaggerState is added.
///
/// Listens to `Added<StaggerState>` to trigger stagger reaction:
/// - Plays "RESET" animation (temp, later will be dedicated stagger animation)
/// - Interrupts any ongoing attack animation
pub fn execute_stagger_animations_main_thread(
    query: Query<Entity, Added<voidrun_simulation::combat::StaggerState>>,
    visuals: NonSend<VisualRegistry>,
) {
    for entity in query.iter() {
        // Get staggered entity node
        let Some(node) = visuals.visuals.get(&entity) else {
            continue;
        };

        // Get MeleeSwingAnimationPlayer (attack animations)
        let Some(mut anim_player) = node
            .try_get_node_as::<godot::classes::AnimationPlayer>("MeleeSwingAnimationPlayer")
        else {
            voidrun_simulation::log(&format!(
                "⚠️ Godot: Entity {:?} has no MeleeSwingAnimationPlayer for stagger interrupt",
                entity
            ));
            continue;
        };

        // Interrupt attack animation → play RESET
        anim_player.set_speed_scale(1.0);
        anim_player.play_ex().name("RESET").done();

        voidrun_simulation::log(&format!(
            "💫 Godot: Stagger animation (entity: {:?}) - attack interrupted, playing RESET",
            entity
        ));
    }
}
