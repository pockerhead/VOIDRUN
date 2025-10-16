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
    /// Отступать от entity (движение назад, но смотреть на target)
    ///
    /// Тактическое отступление:
    /// - Двигаемся в направлении от target (пятиться назад)
    /// - Rotation направлен НА target (смотрим на врага)
    /// - NavigationAgent не используется (прямое управление velocity)
    RetreatFrom { target: Entity },
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

    /// true когда NavigationAgent может достичь target позиции
    pub can_reach_target: bool,

    /// Текущая adjusted distance для FollowEntity (итеративно уменьшается при LOS blocked)
    ///
    /// Логика:
    /// - None: не инициализирована (первый кадр FollowEntity или смена target)
    /// - Some(distance): текущая distance, уменьшается при LOS blocked
    /// - Сбрасывается в None при смене target entity
    pub current_follow_distance: Option<f32>,
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

/// Event: намерение прыгнуть (jump intent)
///
/// Генерируется:
/// - Player input system (Space key)
/// - AI system (для NPC, если нужно)
///
/// Обрабатывается:
/// - apply_safe_velocity_system (Godot layer): проверяет is_on_floor() и применяет jump velocity
#[derive(Event, Debug, Clone)]
pub struct JumpIntent {
    pub entity: Entity,
}
