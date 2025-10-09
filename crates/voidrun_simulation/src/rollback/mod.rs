//! Rollback marker component
//!
//! Маркирует entities которые будут реплицироваться по сети в будущем.
//! Сейчас используется для детерминистичной симуляции и save/load системы.
//!
//! Архитектура:
//! - Детерминизм: fixed timestep (64Hz), seeded RNG, ordered systems
//! - Entities с Rollback участвуют в snapshot для сохранений
//! - В будущем: client-server репликация (не P2P)

use bevy::prelude::*;

/// Rollback marker component
///
/// Добавляется к entities которые управляются детерминистичной симуляцией
/// и будут сохраняться в snapshot для save/load системы.
///
/// Примеры:
/// - Player/NPC actors — YES (replicated)
/// - UI elements — NO (local только)
/// - Particle effects — NO (visual only)
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Rollback;

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

    /// Test: проверяем что компоненты Clone + Reflect
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
