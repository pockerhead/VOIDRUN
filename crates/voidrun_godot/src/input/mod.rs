//! Player input module
//!
//! Обрабатывает input от игрока и преобразует его в ECS events/commands.
//!
//! # Архитектура
//!
//! ```text
//! Godot Input (keyboard/mouse)
//!     ↓
//! PlayerInputController (Godot node) - controller.rs
//!     ↓
//! PlayerInputEvent (ECS event) - events.rs
//!     ↓
//! Player input systems (ECS) - systems.rs
//!     ↓
//! MovementCommand / Combat events
//! ```
//!
//! # Компоненты модуля
//!
//! - `events` - ECS события (PlayerInputEvent)
//! - `systems` - ECS системы обработки input
//! - `controller` - Godot node для чтения Input API

pub mod events;
pub mod systems;
pub mod controller;

// Re-exports для external use
pub use events::*;
pub use systems::*;
pub use controller::*;
