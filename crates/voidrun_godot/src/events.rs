//! Godot tactical layer events
//!
//! События специфичные для Godot presentation layer (не нужны в voidrun_simulation).
//! Регистрируются и обрабатываются только в voidrun_godot.

use bevy::prelude::*;

/// Safe velocity рассчитанная NavigationAgent3D с avoidance
///
/// Flow:
/// 1. apply_navigation_velocity вызывает nav_agent.set_velocity(desired_velocity)
/// 2. NavigationServer3D рассчитывает safe_velocity с учётом других агентов
/// 3. Signal velocity_computed → AvoidanceReceiver → SafeVelocityComputed event
/// 4. apply_safe_velocity_system применяет safe_velocity к CharacterBody3D
///
/// КРИТИЧНО: Это Godot-специфичный event (не нужен в simulation layer)
#[derive(Event, Debug, Clone)]
pub struct SafeVelocityComputed {
    pub entity: Entity,
    pub safe_velocity: Vec3, // Velocity с учётом obstacle avoidance
    pub desired_velocity: Vec3, // Исходная velocity (для debug логирования)
}
