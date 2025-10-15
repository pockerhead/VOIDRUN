//! Collision Layers Constants
//!
//! Godot Physics Layers — centralised constants для всего проекта.
//!
//! ## Архитектура:
//! - **Layers (битовая маска):** На каком слое находится объект
//! - **Mask (битовая маска):** С какими слоями объект коллидирует
//!
//! ## Godot Layers (1-32):
//! - Layer 1 (0b1 = 1): Reserved
//! - Layer 2 (0b10 = 2): Actors (CharacterBody3D)
//! - Layer 3 (0b100 = 4): Environment (StaticBody3D obstacles/walls)
//! - Layer 4 (0b1000 = 8): Projectiles (CharacterBody3D bullets)
//!
//! ## Использование:
//! ```rust
//! use crate::collision_layers::*;
//!
//! // Actor setup
//! actor_node.set_collision_layer(COLLISION_LAYER_ACTORS);
//! actor_node.set_collision_mask(COLLISION_MASK_ACTORS);
//!
//! // Projectile setup
//! projectile.set_collision_layer(COLLISION_LAYER_PROJECTILES);
//! projectile.set_collision_mask(COLLISION_MASK_PROJECTILES);
//!
//! // Raycast setup (actors + environment)
//! query.set_collision_mask(COLLISION_MASK_ACTORS | COLLISION_MASK_ENVIRONMENT);
//! ```

// ============================================================================
// Layer Битовые Маски (на каком слое объект находится)
// ============================================================================

/// Layer 2: Actors (CharacterBody3D — players, NPCs)
pub const COLLISION_LAYER_ACTORS: u32 = 0b10; // 2

/// Layer 3: Environment (StaticBody3D — walls, obstacles, terrain)
pub const COLLISION_LAYER_ENVIRONMENT: u32 = 0b100; // 4

/// Layer 4: Projectiles (CharacterBody3D — bullets)
pub const COLLISION_LAYER_PROJECTILES: u32 = 0b1000; // 8

// ============================================================================
// Mask Битовые Маски (с чем объект коллидирует)
// ============================================================================

/// Mask: Actors collide with Actors + Environment
///
/// Используется для CharacterBody3D actors (player/NPC movement).
pub const COLLISION_MASK_ACTORS: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT;

/// Mask: Projectiles collide with Actors + Environment
///
/// Используется для снарядов (bullets).
/// НЕ коллидируют с другими projectiles (слой 4 отсутствует в маске).
pub const COLLISION_MASK_PROJECTILES: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT;

/// Mask: Raycast для LOS check (Actors + Environment)
///
/// Используется для line-of-sight проверок (AI, weapons).
pub const COLLISION_MASK_RAYCAST_LOS: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT;

// ============================================================================
// Helper Functions
// ============================================================================

/// Получить название слоя для debug логов
pub fn get_layer_name(layer_bits: u32) -> &'static str {
    match layer_bits {
        COLLISION_LAYER_ACTORS => "Actors",
        COLLISION_LAYER_ENVIRONMENT => "Environment",
        COLLISION_LAYER_PROJECTILES => "Projectiles",
        _ => "Unknown",
    }
}
