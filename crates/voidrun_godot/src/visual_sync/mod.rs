//! Visual sync domain — ECS Changed<T> → Godot visual updates
//!
//! Architecture: ADR-004 (NonSend resources + Changed<T> queries)
//! All systems — main thread only (NonSendMut<VisualRegistry>)

mod spawn;
mod labels;
mod lifecycle;

pub use spawn::*;
pub use labels::*;
pub use lifecycle::*;
