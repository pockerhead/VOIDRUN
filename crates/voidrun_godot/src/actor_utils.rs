//! Actor utility functions (Godot tactical layer)
//!
//! Reusable spatial queries for actor interactions:
//! - Mutual facing detection (melee, dialogue, stealth)
//! - Line-of-sight checks
//! - Distance calculations

use godot::prelude::*;

/// Check if two actors are facing each other (mutual facing check)
///
/// Returns `true` if BOTH actors are facing each other within specified angle cone.
///
/// **Use cases:**
/// - Melee windup detection (both see attack coming)
/// - Dialogue interaction (NPCs talk face-to-face)
/// - Stealth detection (guard facing player)
/// - Block/parry mechanics (defender sees attacker)
///
/// **Parameters:**
/// - `actor_a_node`: First actor's Godot Node3D
/// - `actor_b_node`: Second actor's Godot Node3D
/// - `angle_threshold`: Cosine of max angle (e.g., 0.819 = 35°, 0.707 = 45°, 0.5 = 60°)
///
/// **Returns:**
/// - `Some((dot_a, dot_b))`: Both actors facing each other (dot products ≥ threshold)
/// - `None`: One or both actors NOT facing each other
///
/// **Note:** Godot actors face **-Z axis** (basis.col_c() negated)
///
/// # Examples
///
/// ```rust
/// // Melee windup detection (35° cone, tight)
/// if actors_facing_each_other(&attacker, &defender, 0.819).is_some() {
///     ai_events.write(GodotAIEvent::EnemyWindupVisible { ... });
/// }
///
/// // Dialogue interaction (60° cone, relaxed)
/// if actors_facing_each_other(&npc, &player, 0.5).is_some() {
///     start_dialogue(npc, player);
/// }
/// ```
pub fn actors_facing_each_other(
    actor_a_node: &Gd<godot::classes::Node3D>,
    actor_b_node: &Gd<godot::classes::Node3D>,
    angle_threshold: f32,
) -> Option<(f32, f32)> {
    let pos_a = actor_a_node.get_global_position();
    let pos_b = actor_b_node.get_global_position();

    // Forward vectors (Godot actors face -Z)
    let forward_a = -actor_a_node.get_global_transform().basis.col_c();
    let forward_b = -actor_b_node.get_global_transform().basis.col_c();

    // Direction vectors
    let to_b = (pos_b - pos_a).normalized();
    let to_a = (pos_a - pos_b).normalized();

    // Dot products (how much each actor faces the other)
    let dot_a = forward_a.dot(to_b);
    let dot_b = forward_b.dot(to_a);

    // Both must be facing each other
    if dot_a >= angle_threshold && dot_b >= angle_threshold {
        Some((dot_a, dot_b))
    } else {
        None
    }
}

/// Common angle thresholds (cosine values)
pub mod angles {
    /// 30° cone (very tight, almost straight line)
    pub const TIGHT_30_DEG: f32 = 0.866;

    /// 35° cone (tight, melee combat default)
    pub const TIGHT_35_DEG: f32 = 0.819;

    /// 45° cone (moderate, dialogue/interaction)
    pub const MODERATE_45_DEG: f32 = 0.707;

    /// 60° cone (wide, peripheral vision)
    pub const WIDE_60_DEG: f32 = 0.5;

    /// 90° cone (very wide, general awareness)
    pub const VERY_WIDE_90_DEG: f32 = 0.0;
}
