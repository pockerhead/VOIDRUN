//! Movement компоненты: навигация, скорость, команды перемещения

use bevy::prelude::*;

/// Команда движения для актора (выполняется Godot NavigationAgent)
///
/// Архитектура:
/// - ECS система пишет MovementCommand (high-level intent)
/// - Godot система читает и конвертирует в NavigationAgent target
/// - CharacterBody3D применяет физику движения
#[derive(Component, Debug, Clone, PartialEq)]
pub enum MovementCommand {
    /// Стоять на месте (не обновлять NavigationAgent target)
    Idle,
    /// Двигаться к позиции (world coordinates)
    MoveToPosition { target: Vec3 },
    /// Следовать за entity (обновлять target каждый frame)
    FollowEntity { target: Entity },
    /// Остановиться немедленно (сбросить velocity)
    Stop,
}

impl Default for MovementCommand {
    fn default() -> Self {
        Self::Idle
    }
}

/// Состояние навигации актора (для избежания спама PositionChanged events)
///
/// Проблема:
/// - NavigationAgent.is_target_reached() == true каждый frame когда стоим на месте
/// - Если отправлять PositionChanged event каждый frame → спам в ECS
///
/// Решение:
/// - Флаг is_target_reached трекает ПЕРЕХОД unreached → reached
/// - Отправляем event только ОДИН РАЗ при достижении цели
/// - Сбрасываем флаг при новом MovementCommand
///
/// Логика сброса флага:
/// - MoveToPosition: всегда сбрасывать при новом target
/// - FollowEntity: сбрасывать при смене entity ИЛИ если target отошёл > threshold
/// - Idle/Stop: НЕ трогать флаг (сохраняем историю)
#[derive(Component, Default, Clone, Debug)]
pub struct NavigationState {
    /// true когда NavigationAgent достиг target позиции
    /// (используется для one-time PositionChanged event)
    pub is_target_reached: bool,

    /// Последний target entity для FollowEntity (трекаем смену цели)
    pub last_follow_target: Option<Entity>,
}

/// Скорость движения актора (метры/сек)
///
/// Будет использоваться Godot NavigationAgent для расчёта velocity
#[derive(Component, Clone, Copy, Debug)]
pub struct MovementSpeed {
    pub speed: f32,
}

impl Default for MovementSpeed {
    fn default() -> Self {
        Self { speed: 2.0 } // 2 m/s — базовая скорость ходьбы
    }
}
