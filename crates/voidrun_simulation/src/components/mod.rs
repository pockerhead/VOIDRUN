//! ECS Components — backward compatibility re-exports
//!
//! После Phase 1 рефакторинга все компоненты перенесены в domain модули:
//! - actor domain: Actor, Health, Stamina, PlayerControlled
//! - movement domain: MovementCommand, NavigationState, MovementSpeed, JumpIntent
//! - shooting domain: AimMode, ToggleADSIntent
//! - shared domain: StrategicPosition, PrefabPath, EquippedWeapons, Armor, EnergyShield, Inventory, CameraMode, ActiveCamera, Attachment
//! - combat domain: WeaponStats, MeleeAttackState, etc. (уже в combat/)
//! - ai domain: AIState, AIConfig, etc. (уже в ai/)
//!
//! Этот модуль re-export'ит всё из доменов для обратной совместимости.
//! Legacy код может использовать `use voidrun_simulation::components::*;`

// Re-exports из domain modules
pub use crate::actor::*;
pub use crate::movement::*;
pub use crate::shooting::*;
pub use crate::shared::*;
pub use crate::combat::*;
pub use crate::ai::*;
