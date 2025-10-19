//! Equipment system components
//!
//! # Архитектура
//!
//! **EquippedWeapons** — оружие в руках (hotkeys 1-4):
//! - 4 слота: PrimaryLarge1, PrimaryLarge2, SecondarySmall1, SecondarySmall2
//! - `active_slot` (0-3) указывает какой сейчас в руках
//! - При equip → добавляем `WeaponStats` + `Attachment` компоненты
//! - При unequip → удаляем компоненты, возвращаем в Inventory
//!
//! **ConsumableSlots** — быстрый доступ (hotkeys 5-9):
//! - 5 слотов (базовые 2 всегда unlocked)
//! - Слоты 3-5 unlock через armor bonus
//! - Instant use (no equip/unequip)
//!
//! **Armor** — пассивная защита + визуал:
//! - Defense rating (damage reduction)
//! - Consumable slot bonus (unlock 7-9 hotkeys)
//! - При equip → добавляем `Attachment` для визуала
//!
//! **EnergyShield** — энергобарьер:
//! - Блокирует только ranged урон (velocity > threshold)
//! - Melee проходит сквозь щит (slow kinetic)
//! - Recharge delay после получения урона
//!
//! **Inventory** — общая свалка:
//! - Unlimited capacity (пока)
//! - Weight/volume limits позже

use bevy::prelude::*;
use crate::item_system::{ItemId, ItemInstance};

// ============================================================================
// EquippedWeapons (slots 1-4)
// ============================================================================

/// Equipped weapons component (hotkeys 1-4)
///
/// # Weapon slots
/// - [1] PrimaryLarge1 (винтовка, меч)
/// - [2] PrimaryLarge2 (второе large weapon)
/// - [3] SecondarySmall1 (пистолет, кинжал)
/// - [4] SecondarySmall2 (второе small weapon)
///
/// # Active slot
/// - `active_slot` (0-3) указывает какой weapon сейчас в руках
/// - Только активное оружие имеет `WeaponStats` + `Attachment` компоненты
/// - Swap → detach старое + attach новое
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct EquippedWeapons {
    // Weapon slots
    pub primary_large_1: Option<EquippedItem>,
    pub primary_large_2: Option<EquippedItem>,
    pub secondary_small_1: Option<EquippedItem>,
    pub secondary_small_2: Option<EquippedItem>,

    /// Active slot (0-3 = какой weapon в руках)
    pub active_slot: u8,
}

impl Default for EquippedWeapons {
    fn default() -> Self {
        Self {
            primary_large_1: None,
            primary_large_2: None,
            secondary_small_1: None,
            secondary_small_2: None,
            active_slot: 0,
        }
    }
}

impl EquippedWeapons {
    /// Создать пустой equipment
    pub fn empty() -> Self {
        Self::default()
    }

    /// Получить item в слоте (immutable)
    pub fn get_slot(&self, index: u8) -> Option<&EquippedItem> {
        match index {
            0 => self.primary_large_1.as_ref(),
            1 => self.primary_large_2.as_ref(),
            2 => self.secondary_small_1.as_ref(),
            3 => self.secondary_small_2.as_ref(),
            _ => None,
        }
    }

    /// Получить item в слоте (mutable)
    pub fn get_slot_mut(&mut self, index: u8) -> Option<&mut EquippedItem> {
        match index {
            0 => self.primary_large_1.as_mut(),
            1 => self.primary_large_2.as_mut(),
            2 => self.secondary_small_1.as_mut(),
            3 => self.secondary_small_2.as_mut(),
            _ => None,
        }
    }

    /// Установить item в слот
    pub fn set_slot(&mut self, index: u8, item: Option<EquippedItem>) {
        match index {
            0 => self.primary_large_1 = item,
            1 => self.primary_large_2 = item,
            2 => self.secondary_small_1 = item,
            3 => self.secondary_small_2 = item,
            _ => {}
        }
    }

    /// Получить активный weapon (immutable)
    pub fn get_active_weapon(&self) -> Option<&EquippedItem> {
        self.get_slot(self.active_slot)
    }

    /// Получить активный weapon (mutable)
    pub fn get_active_weapon_mut(&mut self) -> Option<&mut EquippedItem> {
        self.get_slot_mut(self.active_slot)
    }

    /// Проверить что слот пустой
    pub fn is_slot_empty(&self, index: u8) -> bool {
        self.get_slot(index).is_none()
    }

    /// Проверить что активный slot пустой
    pub fn is_active_slot_empty(&self) -> bool {
        self.get_active_weapon().is_none()
    }
}

/// Equipped item (runtime state)
///
/// Хранится в `EquippedWeapons` slots.
/// Mutable state (durability, ammo).
#[derive(Clone, Debug, Reflect)]
pub struct EquippedItem {
    /// Ссылка на definition
    pub definition_id: ItemId,
    /// Runtime durability (0.0-1.0)
    pub durability: f32,
    /// Runtime ammo count (для ranged weapons)
    pub ammo_count: Option<u32>,
}

// ============================================================================
// ConsumableSlots (slots 5-9)
// ============================================================================

/// Consumable slots component (hotkeys 5-9)
///
/// # Slots
/// - [5-6] Базовые слоты (всегда unlocked)
/// - [7-9] Unlock через armor bonus
///
/// # Unlock logic
/// - Без брони: 2 слота
/// - Light armor: 3 слота
/// - Tactical armor: 4 слота
/// - Military armor: 5 слотов
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct ConsumableSlots {
    /// 5 слотов для consumables (hotkeys 5-9)
    pub slots: [Option<ItemInstance>; 5],
    /// Количество разблокированных слотов (2-5)
    pub unlocked_count: u8,
}

impl Default for ConsumableSlots {
    fn default() -> Self {
        Self {
            slots: Default::default(),
            unlocked_count: 2, // Базовые 2 слота без брони
        }
    }
}

impl ConsumableSlots {
    /// Создать с базовыми слотами (2 unlocked)
    pub fn empty() -> Self {
        Self::default()
    }

    /// Проверить что слот разблокирован
    pub fn is_slot_unlocked(&self, index: u8) -> bool {
        index < self.unlocked_count
    }

    /// Unlock слоты (через armor)
    pub fn unlock_slots(&mut self, count: u8) {
        self.unlocked_count = count.min(5);
    }

    /// Получить item в слоте (immutable)
    pub fn get_slot(&self, index: u8) -> Option<&ItemInstance> {
        self.slots.get(index as usize)?.as_ref()
    }

    /// Получить item в слоте (mutable)
    pub fn get_slot_mut(&mut self, index: u8) -> Option<&mut ItemInstance> {
        self.slots.get_mut(index as usize)?.as_mut()
    }

    /// Установить item в слот
    pub fn set_slot(&mut self, index: u8, item: Option<ItemInstance>) {
        if let Some(slot) = self.slots.get_mut(index as usize) {
            *slot = item;
        }
    }

    /// Take item из слота (ownership transfer)
    pub fn take_slot(&mut self, index: u8) -> Option<ItemInstance> {
        self.slots.get_mut(index as usize)?.take()
    }
}

// ============================================================================
// Armor
// ============================================================================

/// Armor component (пассивная защита)
///
/// # Lifecycle
/// - При equip: добавляем Armor компонент + Attachment (визуал)
/// - При unequip: удаляем оба компонента
/// - Defense влияет на damage calculation
/// - Consumable slot bonus unlock слоты 7-9
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Armor {
    /// Ссылка на definition
    pub definition_id: ItemId,
    /// Runtime durability (0.0-1.0)
    pub durability: f32,
    /// Defense rating (damage reduction)
    pub defense: u32,
    /// Consumable slot bonus (0-3 доп слота)
    pub consumable_slot_bonus: u8,
}

// ============================================================================
// EnergyShield
// ============================================================================

/// Energy shield component (энергобарьер)
///
/// # Mechanics (из shield-technology.md)
/// - Блокирует только ranged урон (velocity > threshold)
/// - Melee полностью игнорирует щит (slow kinetic)
/// - Recharge delay после получения урона
/// - Recharge rate применяется вне боя
///
/// # Usage
/// - Всегда активен (пассивный компонент)
/// - No equip/unequip (not item)
/// - Faction-based stats (military = лучший щит)
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct EnergyShield {
    /// Max energy
    pub max_energy: f32,
    /// Current energy (0.0 = сломан, max_energy = full)
    pub current_energy: f32,
    /// Recharge rate (энергия/сек) вне боя
    pub recharge_rate: f32,
    /// Recharge delay (секунды после получения урона)
    pub recharge_delay: f32,
    /// Velocity threshold (м/с) для kinetic filtering
    ///
    /// Ranged projectiles: velocity > threshold → блокируется
    /// Melee attacks: velocity < threshold → проходит сквозь щит
    pub velocity_threshold: f32,
    /// Timer для recharge delay
    pub recharge_timer: f32,
}

impl Default for EnergyShield {
    fn default() -> Self {
        Self {
            max_energy: 100.0,
            current_energy: 100.0,
            recharge_rate: 10.0,
            recharge_delay: 2.0,
            velocity_threshold: 5.0, // 5 м/с threshold
            recharge_timer: 0.0,
        }
    }
}

impl EnergyShield {
    /// Создать shield с кастомными stats
    pub fn new(max_energy: f32, recharge_rate: f32, recharge_delay: f32) -> Self {
        Self {
            max_energy,
            current_energy: max_energy,
            recharge_rate,
            recharge_delay,
            velocity_threshold: 5.0,
            recharge_timer: 0.0,
        }
    }

    /// Military shield preset (лучший)
    pub fn military() -> Self {
        Self::new(500.0, 20.0, 2.0)
    }

    /// Basic shield preset
    pub fn basic() -> Self {
        Self::new(200.0, 10.0, 3.0)
    }

    /// Проверить что shield активен (current > 0)
    pub fn is_active(&self) -> bool {
        self.current_energy > 0.0
    }

    /// Получить урон (уменьшить energy)
    pub fn take_damage(&mut self, damage: f32) {
        self.current_energy -= damage;
        self.current_energy = self.current_energy.max(0.0);
        self.recharge_timer = self.recharge_delay; // Reset recharge delay
    }

    /// Tick recharge system
    pub fn tick(&mut self, delta_time: f32) {
        let mut remaining_time = delta_time;

        // Recharge delay countdown
        if self.recharge_timer > 0.0 {
            let delay_time = self.recharge_timer.min(remaining_time);
            self.recharge_timer -= delay_time;
            remaining_time -= delay_time;
        }

        // Recharge energy (с оставшимся временем после delay)
        if remaining_time > 0.0 && self.current_energy < self.max_energy {
            self.current_energy += self.recharge_rate * remaining_time;
            self.current_energy = self.current_energy.min(self.max_energy);
        }
    }
}

// ============================================================================
// Inventory (общая свалка)
// ============================================================================

/// Inventory component (общий storage)
///
/// # Architecture
/// - Vec<ItemInstance> для хранения items
/// - Unlimited capacity пока (weight/volume позже)
/// - Используется для:
///   - Loot pickup
///   - Crafting materials
///   - Quest items
///   - Unequipped weapons/armor
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Inventory {
    /// Items в инвентаре
    pub items: Vec<ItemInstance>,
    /// Capacity (unlimited пока)
    pub capacity: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::empty()
    }
}

impl Inventory {
    /// Создать пустой inventory
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            capacity: usize::MAX, // Unlimited пока
        }
    }

    /// Добавить item
    pub fn add_item(&mut self, item: ItemInstance) {
        self.items.push(item);
    }

    /// Удалить item по индексу
    pub fn remove_item(&mut self, index: usize) -> Option<ItemInstance> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }

    /// Найти item по definition_id
    pub fn find_item(&self, definition_id: &ItemId) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.definition_id == *definition_id)
    }

    /// Проверить что inventory пустой
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Количество items
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipped_weapons_default() {
        let weapons = EquippedWeapons::default();
        assert_eq!(weapons.active_slot, 0);
        assert!(weapons.is_slot_empty(0));
        assert!(weapons.is_active_slot_empty());
    }

    #[test]
    fn test_equipped_weapons_set_get_slot() {
        let mut weapons = EquippedWeapons::empty();

        let sword = EquippedItem {
            definition_id: "melee_sword".into(),
            durability: 1.0,
            ammo_count: None,
        };

        weapons.set_slot(0, Some(sword.clone()));
        assert!(!weapons.is_slot_empty(0));
        assert_eq!(weapons.get_slot(0).unwrap().definition_id, "melee_sword".into());
    }

    #[test]
    fn test_consumable_slots_unlock() {
        let mut slots = ConsumableSlots::empty();
        assert_eq!(slots.unlocked_count, 2);
        assert!(slots.is_slot_unlocked(0));
        assert!(slots.is_slot_unlocked(1));
        assert!(!slots.is_slot_unlocked(2));

        slots.unlock_slots(5);
        assert_eq!(slots.unlocked_count, 5);
        assert!(slots.is_slot_unlocked(4));
    }

    #[test]
    fn test_energy_shield_damage_recharge() {
        let mut shield = EnergyShield::basic();
        assert!(shield.is_active());
        assert_eq!(shield.current_energy, 200.0);

        // Take damage
        shield.take_damage(50.0);
        assert_eq!(shield.current_energy, 150.0);
        assert_eq!(shield.recharge_timer, 3.0);

        // Tick (recharge delay)
        shield.tick(2.0);
        assert_eq!(shield.recharge_timer, 1.0);
        assert_eq!(shield.current_energy, 150.0); // No recharge yet

        // Tick (start recharge)
        shield.tick(1.5);
        assert_eq!(shield.recharge_timer, 0.0);
        // 0.5s recharge: 150.0 + 10.0 * 0.5 = 155.0
        assert!((shield.current_energy - 155.0).abs() < 0.01);
    }

    #[test]
    fn test_inventory_add_remove() {
        let mut inv = Inventory::empty();
        assert!(inv.is_empty());

        let sword = ItemInstance::new("melee_sword");
        inv.add_item(sword);
        assert_eq!(inv.len(), 1);

        let item = inv.remove_item(0);
        assert!(item.is_some());
        assert!(inv.is_empty());
    }

    #[test]
    fn test_inventory_find_item() {
        let mut inv = Inventory::empty();
        inv.add_item(ItemInstance::new("melee_sword"));
        inv.add_item(ItemInstance::new("pistol_basic"));

        let index = inv.find_item(&"pistol_basic".into());
        assert_eq!(index, Some(1));

        let index = inv.find_item(&"unknown".into());
        assert_eq!(index, None);
    }
}
