//! Shared domain — cross-cutting компоненты
//!
//! Содержит компоненты используемые в нескольких доменах:
//! - World positioning (StrategicPosition, PrefabPath)
//! - Equipment (EquippedWeapons, Armor, EnergyShield, Inventory)
//! - Camera (CameraMode, ActiveCamera)
//! - Attachments (Attachment, AttachmentType, DetachAttachment)

pub mod world;
pub mod equipment;
pub mod camera;
pub mod attachment;

// Re-export all components
pub use world::*;
pub use equipment::*;
pub use camera::*;
pub use attachment::*;
