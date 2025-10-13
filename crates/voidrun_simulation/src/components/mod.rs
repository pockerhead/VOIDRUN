//! ECS Components для игровых entity
//!
//! Организация по доменам:
//! - actor: базовые характеристики (faction, health, stamina)
//! - combat: боевая механика (attacker)
//! - movement: навигация и перемещение (MovementCommand, NavigationState)
//! - ai: искусственный интеллект (AIState, AIConfig, SpottedEnemies)
//! - world: позиционирование в мире (StrategicPosition, PrefabPath)
//! - attachment: динамические префабы (Attachment, AttachmentType)

mod actor;
mod combat;
mod movement;
mod ai;
mod world;
mod attachment;

// Re-exports для удобного импорта
pub use actor::*;
pub use combat::*;
pub use movement::*;
pub use ai::*;
pub use world::*;
pub use attachment::*;
