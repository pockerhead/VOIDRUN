//! ECS systems registration
//!
//! Регистрация всех Bevy ECS систем в schedules (Main, Update, FixedUpdate, SlowUpdate, CombatUpdate).

use crate::schedules::{CombatUpdate, FixedTickCounter, SlowUpdate};
use bevy::prelude::*;

/// Регистрация всех ECS систем в Bevy App
pub fn register_systems(app: &mut App) {
    use crate::systems::{
        ai_melee_combat_decision_main_thread, // Unified AI melee combat decision system (attack/parry/wait)
        apply_gravity_to_all_actors,          // Gravity + jump для ВСЕХ акторов (ПЕРВАЯ система!)
        apply_navigation_velocity_main_thread,
        apply_retreat_velocity_main_thread,
        apply_safe_velocity_system, // NavigationAgent3D avoidance
        attach_prefabs_main_thread,
        despawn_actor_visuals_main_thread,
        detach_prefabs_main_thread,
        disable_collision_on_death_main_thread,
        execute_melee_attacks_main_thread,
        execute_parry_animations_main_thread,
        execute_stagger_animations_main_thread,
        poll_melee_hitboxes_main_thread,
        poll_vision_cones_main_thread,
        process_godot_projectile_hits,
        process_melee_attack_intents_main_thread,
        process_movement_commands_main_thread,
        process_ranged_attack_intents_main_thread,
        spawn_actor_visuals_main_thread,
        sync_ai_state_labels_main_thread,
        sync_health_labels_main_thread,
        sync_stamina_labels_main_thread,
        update_combat_targets_main_thread, // Dynamic target switching (closest spotted enemy)
        update_follow_entity_targets_main_thread,
        weapon_aim_main_thread,
        weapon_fire_main_thread,
    };

    // 1. Регистрируем Godot tactical layer events
    app.add_event::<crate::events::SafeVelocityComputed>();
    app.add_event::<voidrun_simulation::JumpIntent>();
    app.add_event::<crate::input::PlayerInputEvent>(); // Player input events

    // 2. Main schedule (spawn/attach/detach prefabs)
    // ВАЖНО: attach_prefabs ПОСЛЕ spawn_actor_visuals (иначе entity не в VisualRegistry!)
    app.add_systems(
        Main,
        (
            spawn_actor_visuals_main_thread,
            attach_prefabs_main_thread,
            detach_prefabs_main_thread,
        )
            .chain(),
    );

    // 3. Update schedule - Movement chain (gravity → nav velocity → safe velocity)
    app.add_systems(
        Update,
        (
            apply_gravity_to_all_actors,            // 1. Gravity + jump для ВСЕХ акторов (ПЕРВАЯ!)
            apply_navigation_velocity_main_thread,  // 2. nav_agent.set_velocity(desired) → velocity_computed signal
            apply_safe_velocity_system,             // 3. SafeVelocityComputed event → CharacterBody3D (AFTER nav velocity)
        )
            .chain(),
    );

    // 4. Update schedule - Input + Labels + Death handling
    app.add_systems(
        Update,
        (
            crate::input::process_player_input,       // Player input → MovementCommand + JumpIntent
            crate::input::player_combat_input,        // Player input → MeleeAttackIntent
            process_movement_commands_main_thread,    // MovementCommand → NavigationAgent3D
            update_follow_entity_targets_main_thread, // Update FollowEntity targets every frame
            apply_retreat_velocity_main_thread,       // RetreatFrom → backpedal + face target
            sync_health_labels_main_thread,
            sync_stamina_labels_main_thread,
            sync_ai_state_labels_main_thread,
            disable_collision_on_death_main_thread, // Отключение collision + gray + DespawnAfter
            despawn_actor_visuals_main_thread, // Удаление Godot nodes для despawned entities
            weapon_aim_main_thread,            // Aim RightHand at target
        ),
    );

    // 5. Update schedule - Combat systems
    app.add_systems(
        Update,
        (
            weapon_aim_main_thread,            // Aim RightHand at target
            process_ranged_attack_intents_main_thread, // WeaponFireIntent → tactical validation → WeaponFired
            weapon_fire_main_thread,                 // WeaponFired → spawn GodotProjectile
            process_godot_projectile_hits,           // Godot queue → ECS ProjectileHit events
            ai_melee_combat_decision_main_thread, // Unified AI melee combat decision (attack/parry/wait)
            process_melee_attack_intents_main_thread, // MeleeAttackIntent → tactical validation → MeleeAttackStarted
            execute_melee_attacks_main_thread, // MeleeAttackState phases → animation + hitbox
            execute_parry_animations_main_thread, // ParryState changed → play melee_parry/melee_parry_recover animations
            execute_stagger_animations_main_thread, // StaggerState added → interrupt attack, play RESET
            poll_melee_hitboxes_main_thread, // Poll hitbox overlaps during ActiveHitbox phase → MeleeHit events
        ),
    );

    // 6. SlowUpdate schedule (3 Hz = ~3 раза в секунду)
    // Для систем с "человеческим временем реакции" (target switching, decision making)
    app.add_systems(
        SlowUpdate,
        (
            poll_vision_cones_main_thread,     // VisionCone → GodotAIEvent
            update_combat_targets_main_thread, // Dynamic target switching (closest visible spotted enemy)
        )
            .chain(),
    );
}

/// Регистрация custom schedules + timer systems
pub fn register_schedules(app: &mut App) {
    use crate::schedules::timer_systems::{
        increment_tick_counter, run_combat_update_timer, run_slow_update_timer,
    };

    // 1. Создаём custom schedules + FixedTickCounter resource (tick-based timing)
    app.init_schedule(SlowUpdate); // 3 Hz (vision, target switching)
    app.init_schedule(CombatUpdate); // 10 Hz (windup detection)
    app.insert_resource(FixedTickCounter::default());

    // 2. Регистрируем timer systems в FixedUpdate (запускаются ПЕРВЫМИ, .chain() для порядка!)
    app.add_systems(
        FixedUpdate,
        (
            increment_tick_counter,     // 1. Increment tick ПЕРВЫМ
            run_slow_update_timer,      // 2. Check SlowUpdate timer (exclusive)
            run_combat_update_timer,    // 3. Check CombatUpdate timer (exclusive)
        )
            .chain(),
    );
}
