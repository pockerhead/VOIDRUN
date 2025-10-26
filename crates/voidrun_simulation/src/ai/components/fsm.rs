//! FSM AI components (state machine, config, spotted enemies).

use bevy::prelude::*;

/// AI FSM состояния (event-driven)
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum AIState {
    /// Idle — начальное состояние после спавна
    Idle,

    /// Patrol — случайное движение в поисках врагов
    Patrol {
        /// Время до следующей смены направления
        next_direction_timer: f32,
        /// Текущая target позиция патруля (генерируется случайно)
        target_position: Option<Vec3>,
    },

    /// Combat — бой с обнаруженным врагом
    Combat {
        target: Entity,
    },

    /// Retreat — отступление для восстановления
    Retreat {
        /// Время отступления (секунды)
        timer: f32,
        /// От кого отступаем (опционально)
        from_target: Option<Entity>,
    },

    /// Dead — актёр мертв (HP == 0), AI отключен
    Dead,
}

impl Default for AIState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Component: tracking spotted enemies (от GodotAIEvent)
///
/// Обновляется через ActorSpotted/ActorLost events.
/// AI использует для выбора target из множества spotted врагов.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct SpottedEnemies {
    pub enemies: Vec<Entity>,
}

/// Параметры AI
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AIConfig {
    /// Stamina порог для отступления (percent)
    pub retreat_stamina_threshold: f32,
    /// Health порог для отступления (percent)
    pub retreat_health_threshold: f32,
    /// Время отступления (секунды)
    pub retreat_duration: f32,
    /// Patrol: время между сменой направления (секунды)
    pub patrol_direction_change_interval: f32,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            retreat_stamina_threshold: 0.3, // 30% stamina
            retreat_health_threshold: 0.2,  // 20% health
            retreat_duration: 2.0,
            patrol_direction_change_interval: 10.0, // Каждые 10 сек новое направление (было 3 сек)
        }
    }
}
