//! AI компоненты: state machine, config, vision

// NOTE: AIState и AIConfig уже определены в crate::ai module
// Экспортируем их здесь для единообразия, но они живут в ai/simple_fsm.rs

// Re-export из ai module (избегаем дублирования)
pub use crate::ai::{AIConfig, AIState, SpottedEnemies};
