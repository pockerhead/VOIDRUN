//! Movement system — обработка MovementCommand → NavigationAgent3D
//!
//! Architecture: ADR-004 (Changed<MovementCommand> → Godot NavigationAgent)
//! Main thread only (Godot API)
//!
//! ВАЖНО: NavigationAgent3D паттерн (упрощённый, без avoidance):
//! 1. Устанавливаем target_position при изменении MovementCommand
//! 2. Каждый frame: берём get_next_path_position() от NavigationAgent
//! 3. Вычисляем направление к waypoint
//! 4. Применяем velocity к CharacterBody3D напрямую (без avoidance)
//!
//! ПОЧЕМУ НЕ velocity_computed callback:
//! - Требует avoidance_enabled = true
//! - Сложная интеграция с ECS (нужен wrapper class или untyped connect)
//! - Для single-player достаточно простого pathfinding без obstacle avoidance

pub mod commands;
pub mod navigation;
pub mod velocity;

// Re-export all systems
pub use commands::*;
pub use navigation::*;
pub use velocity::*;
