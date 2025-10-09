//! Rollback netcode infrastructure
//!
//! Архитектура:
//! - GGRS (Good Game Rollback System) для P2P deterministic rollback
//! - Snapshot/Restore mechanism для всех rollback-managed components
//! - Rollback marker component для entities которые участвуют в rollback
//!
//! Требования:
//! - Все rollback components должны быть Clone + Reflect
//! - Детерминизм: fixed timestep (64Hz), seeded RNG, ordered systems
//! - Checksum validation для desyncs detection

use bevy::prelude::*;

// GGRS configuration (раскомментировать когда bevy_ggrs добавлен в Cargo.toml)
// pub mod ggrs_config;
// pub use ggrs_config::{VoidrunGGRSConfig, encode_input, decode_input, create_world_checksum};

/// Rollback marker component
///
/// Добавляется к entities которые управляются rollback netcode.
/// Entities без Rollback компонента не будут сохраняться в snapshot.
///
/// Примеры:
/// - Player/NPC actors — YES (rollback managed)
/// - UI elements — NO (local только)
/// - Particle effects — NO (visual only)
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Rollback;

/// Регистрирует все rollback components для GGRS
///
/// ВАЖНО: каждый компонент который участвует в rollback должен быть здесь.
/// Раскомментировать когда bevy_ggrs будет добавлен в Cargo.toml
#[allow(dead_code)]
pub fn register_rollback_components(_app: &mut App) {
    // TODO: Раскомментировать когда bevy_ggrs активен
    /*
    // Core components
    app.register_rollback_component::<Transform>();
    app.register_rollback_component::<crate::components::Actor>();
    app.register_rollback_component::<crate::components::Health>();
    app.register_rollback_component::<crate::components::Stamina>();
    app.register_rollback_component::<crate::components::PhysicsBody>();

    // Physics components
    app.register_rollback_component::<crate::physics::KinematicController>();
    app.register_rollback_component::<crate::physics::MovementInput>();

    // Combat components
    app.register_rollback_component::<crate::combat::Attacker>();
    app.register_rollback_component::<crate::combat::AttackHitbox>();
    app.register_rollback_component::<crate::combat::Exhausted>();

    // AI components
    app.register_rollback_component::<crate::ai::AIState>();
    app.register_rollback_component::<crate::ai::AIConfig>();

    // Rollback marker
    app.register_rollback_component::<Rollback>();
    */
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    /// Test: создаем entity с Rollback и проверяем что компоненты добавлены
    #[test]
    fn test_rollback_marker() {
        let mut app = create_headless_app(42);

        let entity = app
            .world_mut()
            .spawn((
                Rollback,
                Transform::default(),
                Actor { faction_id: 1 },
            ))
            .id();

        // Проверяем что entity существует и имеет Rollback
        let world = app.world();
        assert!(world.get::<Rollback>(entity).is_some());
        assert!(world.get::<Transform>(entity).is_some());
        assert!(world.get::<Actor>(entity).is_some());

        // Required components добавлены автоматически
        assert!(world.get::<Health>(entity).is_some());
        assert!(world.get::<Stamina>(entity).is_some());
    }

    /// Test: проверяем что компоненты Clone + Reflect (требование GGRS)
    #[test]
    fn test_components_cloneable() {
        let health = Health::new(100);
        let cloned = health.clone();
        assert_eq!(health.current, cloned.current);

        let stamina = Stamina::new(100.0);
        let cloned = stamina.clone();
        assert_eq!(stamina.current, cloned.current);

        let actor = Actor { faction_id: 42 };
        let cloned = actor.clone();
        assert_eq!(actor.faction_id, cloned.faction_id);
    }
}
