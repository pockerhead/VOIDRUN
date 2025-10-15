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
    WeaponStats,
};

use crate::systems::{VisualRegistry, AttachmentRegistry};

/// System: Process melee attack intents (Godot tactical validation).
///
/// Validates:
/// - Distance check (Godot Transform authoritative)
/// - Line of sight (optional, TODO)
///
/// If validation passes → generates `MeleeAttackStarted` event.
pub fn process_melee_attack_intents_main_thread(
    mut intent_events: EventReader<MeleeAttackIntent>,
    visuals: NonSend<VisualRegistry>,
    weapons: Query<&WeaponStats>,
    mut started_events: EventWriter<MeleeAttackStarted>,
) {
    for intent in intent_events.read() {
        voidrun_simulation::log(&format!("📥 Godot: Received melee intent (attacker: {:?}, target: {:?})", intent.attacker, intent.target));

        // Get Godot nodes
        let Some(attacker_node) = visuals.visuals.get(&intent.attacker) else {
            voidrun_simulation::log(&format!("❌ Godot: attacker {:?} has no visual node", intent.attacker));
            continue;
        };
        let Some(target_node) = visuals.visuals.get(&intent.target) else {
            voidrun_simulation::log(&format!("❌ Godot: target {:?} has no visual node", intent.target));
            continue;
        };

        // Get weapon stats for attack parameters
        let Ok(weapon) = weapons.get(intent.attacker) else {
            voidrun_simulation::log(&format!("❌ Godot: attacker {:?} has no weapon", intent.attacker));
            continue;
        };

        // Tactical validation: distance check
        let attacker_pos = attacker_node.get_global_position();
        let target_pos = target_node.get_global_position();
        let distance = attacker_pos.distance_to(target_pos);

        if distance > weapon.attack_radius {
            // Too far away
            voidrun_simulation::log(&format!("❌ Godot: Melee validation FAILED ({:.2}m > {:.2}m radius)", distance, weapon.attack_radius));
            continue;
        }

        // TODO: Line of sight check (raycast)

        // Validation passed → generate MeleeAttackStarted
        started_events.write(MeleeAttackStarted {
            attacker: intent.attacker,
            target: intent.target,
            attack_type: intent.attack_type.clone(),
            windup_duration: weapon.windup_duration,
            attack_duration: weapon.attack_duration,
            recovery_duration: weapon.recovery_duration,
        });

        voidrun_simulation::log(&format!(
            "⚔️ Godot: Melee attack validated (attacker: {:?}, target: {:?}, distance: {:.2}m)",
            intent.attacker, intent.target, distance
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
        let Some(weapon_attachment) = attachments.attachments.get(&(entity, "RightHand/WeaponAttachment".to_string())) else {
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

            AttackPhase::Active { duration } => {
                // Trigger swing animation + enable hitbox
                if let Some(mut player) = anim_player {
                    let anim_length = get_animation_length(&mut player, "melee_swing");
                    let speed_scale = anim_length / duration;

                    player.set_speed_scale(speed_scale);
                    player.play_ex().name("melee_swing").done();

                    voidrun_simulation::log(&format!(
                        "⚔️ Godot: Playing 'melee_swing' (entity: {:?}, duration: {:.2}s, speed: {:.2}x)",
                        entity, duration, speed_scale
                    ));
                }
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
/// **Anti-spam:** Uses `has_hit_target` flag to ensure only ONE hit per attack.
pub fn poll_melee_hitboxes_main_thread(
    mut query: Query<(Entity, &mut MeleeAttackState)>,
    visuals: NonSend<VisualRegistry>,
    attachments: NonSend<AttachmentRegistry>,
    mut melee_hit_events: EventWriter<voidrun_simulation::combat::MeleeHit>,
) {
    for (attacker, mut attack_state) in query.iter_mut() {
        // Only check during Active phase
        let AttackPhase::Active { .. } = &attack_state.phase else {
            continue;
        };

        // Skip if already hit target this attack (prevent spam!)
        if attack_state.has_hit_target {
            continue;
        }

        // Get weapon attachment
        let Some(weapon_attachment) = attachments.attachments.get(&(attacker, "RightHand/WeaponAttachment".to_string())) else {
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

                    // Only hit intended target (prevent cleave damage for now)
                    if target_entity != attack_state.target {
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

                    // Mark as hit to prevent multiple hits per attack
                    attack_state.has_hit_target = true;

                    voidrun_simulation::log(&format!(
                        "💥 Godot: Melee hit detected! (attacker: {:?}, target: {:?})",
                        attacker, target_entity
                    ));

                    // Break after first hit (no cleave damage)
                    break;
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
