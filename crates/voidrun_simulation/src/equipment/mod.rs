//! Equipment module — lifecycle management
//!
//! # Architecture
//!
//! **Events → Systems flow:**
//! - User/AI emits intent events
//! - Systems process intents (modify components)
//! - Changed<T> triggers visual sync (Godot)
//!
//! **Weapon lifecycle:**
//! - Equip → добавить WeaponStats + Attachment
//! - Unequip → удалить компоненты, вернуть в Inventory
//! - Swap → smooth transition (detach → attach)
//!
//! **Armor lifecycle:**
//! - Equip → добавить Armor + Attachment + unlock consumables
//! - Unequip → удалить компоненты, lock consumables
//!
//! **Consumables:**
//! - Use → instant effect (restore HP/stamina, spawn grenade)

use bevy::prelude::*;

pub mod events;
pub mod systems;

// Re-exports
pub use events::*;
pub use systems::*;

/// Equipment plugin (lifecycle management)
pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app
            // Events
            .add_event::<EquipWeaponIntent>()
            .add_event::<UnequipWeaponIntent>()
            .add_event::<SwapActiveWeaponIntent>()
            .add_event::<EquipArmorIntent>()
            .add_event::<UnequipArmorIntent>()
            .add_event::<UseConsumableIntent>()
            // Systems (обрабатываем в Update schedule)
            .add_systems(Update, (
                process_equip_weapon,
                process_unequip_weapon,
                process_weapon_swap,
                process_equip_armor,
                process_unequip_armor,
                process_use_consumable,
            ));
    }
}
