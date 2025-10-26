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
//! use crate::shared::collision::*;
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

/// Layer 5: Shields (StaticBody3D — energy shield spheres)
pub const COLLISION_LAYER_SHIELDS: u32 = 0b10000; // 16

/// Layer 6: Corpses (dead actors — лежат на земле, не блокируют живых)
pub const COLLISION_LAYER_CORPSES: u32 = 0b100000; // 32

// ============================================================================
// Mask Битовые Маски (с чем объект коллидирует)
// ============================================================================

/// Mask: Actors collide with Actors + Environment
///
/// Используется для CharacterBody3D actors (player/NPC movement).
pub const COLLISION_MASK_ACTORS: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT;

/// Mask: Projectiles collide with Actors + Environment + Shields
///
/// Используется для снарядов (bullets).
/// НЕ коллидируют с другими projectiles (слой 4 отсутствует в маске).
pub const COLLISION_MASK_PROJECTILES: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT | COLLISION_LAYER_SHIELDS;

/// Mask: Raycast для LOS check (Actors + Environment)
///
/// Используется для line-of-sight проверок (AI, weapons).
pub const COLLISION_MASK_RAYCAST_LOS: u32 = COLLISION_LAYER_ACTORS | COLLISION_LAYER_ENVIRONMENT;

/// Mask: Shields DON'T collide actively (passive collision)
///
/// Используется для StaticBody3D shield spheres.
/// Shield НЕ коллидит ни с чем активно (mask = 0), но Projectiles детектируют Shield.
/// Projectiles останавливаются на Shield благодаря своей collision mask.
pub const COLLISION_MASK_SHIELDS: u32 = 0;

/// Mask: Corpses collide with Environment only
///
/// Используется для мёртвых акторов (лежат на земле, но не блокируют живых).
/// НЕ коллидируют с: Actors (layer 2), Projectiles (layer 4), другими Corpses.
pub const COLLISION_MASK_CORPSES: u32 = COLLISION_LAYER_ENVIRONMENT;

// ============================================================================
// Helper Functions
// ============================================================================

/// Получить название слоя для debug логов
pub fn get_layer_name(layer_bits: u32) -> &'static str {
    match layer_bits {
        COLLISION_LAYER_ACTORS => "Actors",
        COLLISION_LAYER_ENVIRONMENT => "Environment",
        COLLISION_LAYER_PROJECTILES => "Projectiles",
        COLLISION_LAYER_SHIELDS => "Shields",
        COLLISION_LAYER_CORPSES => "Corpses",
        _ => "Unknown",
    }
}
