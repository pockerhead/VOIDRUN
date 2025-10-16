//! ECS Components для игровых entity
//!
//! Организация по доменам:
//! - actor: базовые характеристики (faction, health, stamina)
//! - combat: боевая механика (attacker)
//! - movement: навигация и перемещение (MovementCommand, NavigationState)
//! - ai: искусственный интеллект (AIState, AIConfig, SpottedEnemies)
//! - world: позиционирование в мире (StrategicPosition, PrefabPath)
//! - attachment: динамические префабы (Attachment, AttachmentType)
//! - player: player control marker (Player)

pub mod actor;
pub mod combat;
pub mod movement;
pub mod ai;
pub mod world;
pub mod attachment;
pub mod player;

// Re-exports для удобного импорта
pub use actor::*;
pub use combat::*;
pub use movement::*;
pub use ai::*;
pub use world::*;
pub use attachment::*;
pub use player::*;
