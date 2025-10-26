//! Actor domain — базовые компоненты акторов
//!
//! Содержит:
//! - Actor (базовый компонент для NPC/player)
//! - Health (здоровье)
//! - Stamina (выносливость)
//! - PlayerControlled (маркер для игрока)

pub mod components;

// Re-export all components
pub use components::*;
