//! Equipment system events
//!
//! # Architecture
//!
//! **Weapon lifecycle:**
//! - `EquipWeaponIntent` → equip weapon в слот (добавляет WeaponStats + Attachment)
//! - `UnequipWeaponIntent` → unequip weapon (удаляет компоненты, возвращает в Inventory)
//! - `SwapActiveWeaponIntent` → меняет active slot (smooth transition)
//!
//! **Armor lifecycle:**
//! - `EquipArmorIntent` → equip armor (добавляет Armor + Attachment + unlock consumable slots)
//! - `UnequipArmorIntent` → unequip armor (удаляет компоненты, lock consumable slots)
//!
//! **Consumables:**
//! - `UseConsumableIntent` → use consumable из слота (instant effect)

use bevy::prelude::*;
use crate::item_system::ItemInstance;

// ============================================================================
// Weapon Events
// ============================================================================

/// Equip weapon в конкретный слот
///
/// # Flow
/// 1. Unequip старое оружие из слота (если есть)
/// 2. Добавить новое оружие в слот
/// 3. Если это active slot → добавить WeaponStats + Attachment компоненты
#[derive(Event, Clone, Debug)]
pub struct EquipWeaponIntent {
    pub entity: Entity,
    pub slot: WeaponSlot,
    pub item: ItemInstance,
}

/// Unequip weapon из слота
///
/// # Flow
/// 1. Удалить weapon из слота
/// 2. Если это active slot → удалить WeaponStats + Attachment компоненты
/// 3. Вернуть weapon в Inventory
#[derive(Event, Clone, Debug)]
pub struct UnequipWeaponIntent {
    pub entity: Entity,
    pub slot: WeaponSlot,
}

/// Swap активного оружия (hotkeys 1-4)
///
/// # Flow
/// 1. Start holster animation (optional)
/// 2. Detach старый weapon (empty prefab_path → Changed<Attachment>)
/// 3. Update active_slot
/// 4. Attach новый weapon
/// 5. Update WeaponStats компонент
/// 6. Start draw animation (optional)
#[derive(Event, Clone, Debug)]
pub struct SwapActiveWeaponIntent {
    pub entity: Entity,
    pub target_slot: u8, // 0-3 (hotkeys 1-4)
}

/// Weapon slot identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponSlot {
    PrimaryLarge1,    // [1]
    PrimaryLarge2,    // [2]
    SecondarySmall1,  // [3]
    SecondarySmall2,  // [4]
}

impl WeaponSlot {
    /// Convert slot → index (0-3)
    pub fn to_index(self) -> u8 {
        match self {
            WeaponSlot::PrimaryLarge1 => 0,
            WeaponSlot::PrimaryLarge2 => 1,
            WeaponSlot::SecondarySmall1 => 2,
            WeaponSlot::SecondarySmall2 => 3,
        }
    }

    /// Convert index → slot
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(WeaponSlot::PrimaryLarge1),
            1 => Some(WeaponSlot::PrimaryLarge2),
            2 => Some(WeaponSlot::SecondarySmall1),
            3 => Some(WeaponSlot::SecondarySmall2),
            _ => None,
        }
    }
}

// ============================================================================
// Armor Events
// ============================================================================

/// Equip armor
///
/// # Flow
/// 1. Unequip старую броню (если есть)
/// 2. Добавить Armor компонент
/// 3. Добавить Attachment (визуал на %Body)
/// 4. Unlock consumable slots (2 + armor bonus)
#[derive(Event, Clone, Debug)]
pub struct EquipArmorIntent {
    pub entity: Entity,
    pub item: ItemInstance,
}

/// Unequip armor
///
/// # Flow
/// 1. Удалить Armor компонент
/// 2. Удалить Attachment (визуал)
/// 3. Lock consumable slots (обратно к базовым 2)
#[derive(Event, Clone, Debug)]
pub struct UnequipArmorIntent {
    pub entity: Entity,
}

// ============================================================================
// Consumable Events
// ============================================================================

/// Use consumable из слота (hotkeys 5-9)
///
/// # Flow
/// 1. Проверить что слот unlocked
/// 2. Take consumable из слота
/// 3. Apply consumable effect (restore HP/stamina, spawn grenade, etc)
#[derive(Event, Clone, Debug)]
pub struct UseConsumableIntent {
    pub entity: Entity,
    pub slot_index: u8, // 0-4 (hotkeys 5-9)
}
