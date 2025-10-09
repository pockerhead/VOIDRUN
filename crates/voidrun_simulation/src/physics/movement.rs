//! Kinematic контроллер для NPC/игрока
//!
//! Архитектура:
//! - Rapier для коллизий (RigidBody::KinematicPositionBased)
//! - Custom velocity integration (не используем Rapier forces)
//! - Gravity + ground check + movement input
//!
//! Детерминизм: fixed timestep (64Hz), детерминированная физика Rapier

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::PhysicsBody;

/// Kinematic контроллер компонент
///
/// Управляет движением персонажа (WASD + gravity).
/// Использует Rapier для коллизий, но velocity интегрируем сами.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct KinematicController {
    /// Скорость движения (m/s)
    pub move_speed: f32,
    /// Сила гравитации (m/s²)
    pub gravity: f32,
    /// На земле ли персонаж (для прыжков в будущем)
    pub grounded: bool,
}

impl Default for KinematicController {
    fn default() -> Self {
        Self {
            move_speed: 5.0,     // 5 m/s (средняя скорость ходьбы)
            gravity: -9.81,      // Earth gravity
            grounded: false,
        }
    }
}

/// Входные данные для движения (WASD)
///
/// Для headless тестов — mock input через этот компонент.
/// Для игры — заполняется из Input<KeyCode>.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct MovementInput {
    /// Направление движения (normalized)
    pub direction: Vec3,
    /// Jump pressed (для будущего)
    pub jump: bool,
}

/// Система применения gravity к velocity
///
/// Работает в FixedUpdate (64Hz) для детерминизма.
pub fn apply_gravity(
    mut query: Query<(&KinematicController, &mut PhysicsBody)>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (controller, mut body) in query.iter_mut() {
        if !controller.grounded {
            // Применяем гравитацию только если не на земле
            body.velocity.y += controller.gravity * delta;
        }
    }
}

/// Система применения движения от input
///
/// Обновляет velocity на основе MovementInput.
/// Работает в FixedUpdate (64Hz).
pub fn apply_movement_input(
    mut query: Query<(&KinematicController, &MovementInput, &mut PhysicsBody)>,
) {
    for (controller, input, mut body) in query.iter_mut() {
        if input.direction.length_squared() > 0.01 {
            // Normalize input direction
            let direction = input.direction.normalize();

            // Применяем горизонтальную скорость (X, Z)
            // Y velocity остается (gravity handling)
            body.velocity.x = direction.x * controller.move_speed;
            body.velocity.z = direction.z * controller.move_speed;
        } else {
            // Останавливаем горизонтальное движение (трение)
            body.velocity.x = 0.0;
            body.velocity.z = 0.0;
        }
    }
}

/// Система интеграции velocity → position через Rapier
///
/// Rapier автоматически применяет velocity к KinematicPositionBased телам.
/// Эта система только синхронизирует наш PhysicsBody.velocity с Rapier.
pub fn sync_velocity_to_rapier(
    mut query: Query<(&PhysicsBody, &mut Velocity), With<KinematicController>>,
) {
    for (body, mut rapier_velocity) in query.iter_mut() {
        rapier_velocity.linvel = body.velocity;
    }
}

/// Система ground detection через простую Y-проверку
///
/// Stub для Vertical Slice: grounded если y <= 0.5 (пол на y=0, персонаж высотой 1.8m)
///
/// TODO: Заменить на raycast через RapierContext когда подключим полный Rapier plugin
pub fn ground_detection(
    mut query: Query<(&Transform, &mut KinematicController)>,
) {
    for (transform, mut controller) in query.iter_mut() {
        // Простая проверка: если позиция Y близка к полу (y=0) — grounded
        // Персонаж capsule высотой 1.8m, центр на y=0.9, нижняя точка на y=0
        // Считаем grounded если y <= 0.5 (небольшой запас для numerical errors)
        controller.grounded = transform.translation.y <= 0.5;
    }
}

/// Система интеграции velocity → Transform (headless режим, без Rapier)
///
/// Напрямую применяет PhysicsBody.velocity к Transform.translation.
/// Используется когда Rapier не подключен (headless симуляция).
pub fn integrate_velocity_to_transform(
    mut query: Query<(&PhysicsBody, &mut Transform), With<KinematicController>>,
    time: Res<Time<Fixed>>,
) {
    let delta = time.delta_secs();

    for (body, mut transform) in query.iter_mut() {
        // Интегрируем velocity в position: position += velocity * dt
        transform.translation += body.velocity * delta;
    }
}

/// Plugin для kinematic контроллера
///
/// Регистрирует все системы в FixedUpdate для детерминизма.
pub struct KinematicControllerPlugin;

impl Plugin for KinematicControllerPlugin {
    fn build(&self, app: &mut App) {
        use bevy_rapier3d::plugin::PhysicsSet;

        // Наши системы запускаются ДО rapier physics step
        app.add_systems(
            FixedUpdate,
            (
                ground_detection,
                apply_movement_input,
                apply_gravity,
                integrate_velocity_to_transform, // Прямая интеграция (rapier только для collisions)
            )
                .chain() // Последовательное выполнение
                .before(PhysicsSet::SyncBackend), // До rapier physics step
        );
    }
}

/// Spawn helper для создания kinematic персонажа
///
/// Создает entity с полным набором компонентов:
/// - Transform + GlobalTransform
/// - PhysicsBody (custom velocity)
/// - KinematicController
/// - Rapier: RigidBody + Collider (capsule)
pub fn spawn_kinematic_character(
    commands: &mut Commands,
    position: Vec3,
) -> Entity {
    use crate::combat::collision;

    commands
        .spawn((
            // Bevy transform
            Transform::from_translation(position),

            // Наши компоненты
            PhysicsBody::default(),
            KinematicController::default(),
            MovementInput::default(),

            // Rapier physics
            RigidBody::KinematicPositionBased,
            Collider::capsule_y(0.5, 0.4), // Высота 1.0m (0.5 + 0.5), радиус 0.4m
            Velocity::default(),

            // Collision groups (actors коллайдят друг с другом)
            collision::actor_groups(),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gravity_logic() {
        // Тестируем логику гравитации напрямую (без App schedule)
        let controller = KinematicController {
            grounded: false,
            ..default()
        };
        let mut body = PhysicsBody {
            velocity: Vec3::ZERO,
            mass: 70.0,
        };

        let delta = 1.0 / 64.0; // 1 FixedUpdate tick

        // Применяем гравитацию
        if !controller.grounded {
            body.velocity.y += controller.gravity * delta;
        }

        // После 1/64 sec: velocity.y = -9.81 * (1/64) ≈ -0.153
        assert!(body.velocity.y < -0.15);
        assert!(body.velocity.y > -0.16);
    }

    #[test]
    fn test_movement_input_logic() {
        // Тестируем логику движения напрямую
        let controller = KinematicController::default();
        let input = MovementInput {
            direction: Vec3::Z, // Forward
            jump: false,
        };
        let mut body = PhysicsBody::default();

        // Применяем input к velocity
        if input.direction.length_squared() > 0.01 {
            let direction = input.direction.normalize();
            body.velocity.x = direction.x * controller.move_speed;
            body.velocity.z = direction.z * controller.move_speed;
        }

        // Проверяем что velocity.z = move_speed (5.0)
        assert!((body.velocity.z - 5.0).abs() < 0.01, "velocity.z = {}", body.velocity.z);
        assert!((body.velocity.x).abs() < 0.01, "velocity.x = {}", body.velocity.x);
    }

    #[test]
    fn test_grounded_stops_gravity_logic() {
        // Тестируем что grounded предотвращает гравитацию
        let controller = KinematicController {
            grounded: true,
            ..default()
        };
        let mut body = PhysicsBody {
            velocity: Vec3::ZERO,
            mass: 70.0,
        };

        let delta = 1.0 / 64.0;

        // Пытаемся применить гравитацию
        if !controller.grounded {
            body.velocity.y += controller.gravity * delta;
        }

        // Velocity должен остаться 0 (grounded блокирует гравитацию)
        assert_eq!(body.velocity.y, 0.0);
    }
}
