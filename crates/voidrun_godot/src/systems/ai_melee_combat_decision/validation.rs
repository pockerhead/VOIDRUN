//! Tactical validation logic (Godot-dependent checks).
//!
//! Facing, distance, and other spatial checks using Godot nodes.

use godot::prelude::*;

// ============================================================================
// Facing Validation
// ============================================================================

/// Check if defender is facing attacker (front 60° cone).
///
/// Returns true if attacker is in front of defender (dot product > 0.5).
pub(super) fn is_facing_attacker(
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
