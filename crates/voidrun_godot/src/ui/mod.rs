//! UI domain — debug overlays and in-game user interface.
//!
//! # Architecture
//!
//! This domain handles Godot UI layer:
//! - **debug_overlay**: DebugOverlay node (FPS counter, spawn buttons, etc.)
//!
//! # Design Rationale
//!
//! UI is a Godot presentation layer concern:
//! - All UI implemented as Godot nodes (CanvasLayer, Control)
//! - ECS doesn't manage UI state (Godot authoritative)
//! - Debug tools interact with SimulationBridge via node paths
//!
//! # Submodules
//!
//! - `debug_overlay`: DebugOverlay node (FPS, spawn controls, game state display)

pub mod debug_overlay;

// Re-export debug overlay node
pub use debug_overlay::DebugOverlay;
