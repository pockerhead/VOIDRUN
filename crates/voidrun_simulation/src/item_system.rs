//! Item System — базовая инфраструктура для предметов
//!
//! # Архитектура
//!
//! **ItemDefinition** — статический blueprint (id + type + templates):
//! - Хранится в `ItemDefinitions` resource (HashMap lookup)
//! - Immutable данные (name, stats templates, prefab paths)
//! - Создаются hardcoded в `ItemDefinitions::default()` (позже из RON)
//!
//! **ItemInstance** — runtime конкретный предмет:
//! - Ссылается на `ItemDefinition` через `ItemId`
//! - Mutable state (durability, ammo_count, stack_size)
//! - Хранится в `Inventory`, `EquippedWeapons`, `ConsumableSlots`
//!
//! **ItemType** — категории предметов:
//! - Weapon (Large/Small) → EquippedWeapons (slots 1-4)
//! - Consumable → ConsumableSlots (slots 5-9)
//! - Armor → Armor компонент
//! - Shield → физический щит (not EnergyShield!)
//!
//! # Пример использования
//!
//! ```rust
//! // Lookup definition
//! let def = definitions.get(&ItemId("melee_sword".into()))?;
//!
//! // Create instance
//! let sword = ItemInstance {
//!     definition_id: ItemId("melee_sword".into()),
//!     stack_size: 1,
//!     durability: Some(0.8), // 80% durability
//!     ammo_count: None,
//! };
//!
//! // Get weapon template
//! if let Some(template) = &def.weapon_template {
//!     let weapon_stats = template.to_weapon_stats();
//! }
//! ```

use bevy::prelude::*;
use std::collections::HashMap;
use crate::combat::{WeaponStats, WeaponType};

// ============================================================================
// ItemId
// ============================================================================

/// Item identifier (unique string ID)
///
/// # Examples
/// - "melee_sword"
/// - "pistol_basic"
/// - "health_kit"
/// - "armor_military"
#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ItemId(pub String);

impl From<&str> for ItemId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ============================================================================
// ItemType
// ============================================================================

/// Тип предмета (категория)
#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
pub enum ItemType {
    /// Weapon (melee или ranged)
    Weapon { size: WeaponSize },
    /// Armor (визуал + defense + consumable slot bonus)
    Armor,
    /// Physical shield (блокирует удары) — НЕ энергощит!
    Shield,
    /// Consumable (health kit, grenade, etc.)
    Consumable,
    /// Craft material (для крафта)
    CraftMaterial,
    /// Quest item
    Quest,
}

/// Размер оружия (для слотов 1-4)
#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
pub enum WeaponSize {
    /// Large weapon (винтовка, меч, кувалда) — slots 1-2
    Large,
    /// Small weapon (пистолет, кинжал) — slots 3-4
    Small,
}

// ============================================================================
// ItemDefinition (статические данные)
// ============================================================================

/// Static item definition (blueprint)
///
/// Immutable данные, хранятся в `ItemDefinitions` resource.
/// Создаются один раз при запуске игры (hardcoded или из RON).
#[derive(Clone, Debug, Reflect)]
pub struct ItemDefinition {
    /// Unique ID
    pub id: ItemId,
    /// Локализованное название
    pub name: String,
    /// Тип предмета
    pub item_type: ItemType,

    // === Weapon-specific ===
    /// Weapon stats template (для создания WeaponStats компонента)
    pub weapon_template: Option<WeaponStatsTemplate>,
    /// Prefab path для визуала
    pub prefab_path: Option<String>,
    /// Attachment point name
    pub attachment_point: Option<String>,

    // === Armor-specific ===
    /// Armor stats template
    pub armor_stats: Option<ArmorStatsTemplate>,

    // === Consumable-specific ===
    /// Consumable effect
    pub consumable_effect: Option<ConsumableEffect>,
}

// ============================================================================
// WeaponStatsTemplate
// ============================================================================

/// Weapon stats template (для создания WeaponStats компонента)
///
/// Хранится в `ItemDefinition`, конвертируется в `WeaponStats` при equip.
///
/// # Архитектура
/// - Композиция через `WeaponStats` (избегаем дублирования 15 полей)
/// - Template = base stats (immutable)
/// - WeaponStats = base + runtime state (cooldown_timer)
#[derive(Clone, Debug, Reflect)]
pub struct WeaponStatsTemplate {
    /// Base weapon stats (immutable)
    pub stats: WeaponStats,
}

impl WeaponStatsTemplate {
    /// Конвертировать template в WeaponStats компонент
    pub fn to_weapon_stats(&self) -> WeaponStats {
        let mut stats = self.stats.clone();
        stats.cooldown_timer = 0.0; // Reset runtime state
        stats
    }

    /// Melee sword preset
    pub fn melee_sword() -> Self {
        Self {
            stats: WeaponStats::melee_sword(),
        }
    }

    /// Dagger preset (быстрое легкое оружие)
    pub fn dagger() -> Self {
        Self {
            stats: WeaponStats {
                weapon_type: WeaponType::Melee {
                    can_block: false,
                    can_parry: true,
                },
                base_damage: 15,
                attack_cooldown: 0.6,
                cooldown_timer: 0.0,
                attack_radius: 1.5,
                windup_duration: 0.15,
                attack_duration: 0.2,
                recovery_duration: 0.2,
                parry_window: 0.08,
                parry_active_duration: 0.15,
                stagger_duration: 1.0,
                range: 0.0,
                projectile_speed: 0.0,
                hearing_range: 0.0,
            },
        }
    }

    /// Ranged pistol preset
    pub fn ranged_pistol() -> Self {
        Self {
            stats: WeaponStats::ranged_pistol(),
        }
    }

    /// Ranged rifle preset
    pub fn ranged_rifle() -> Self {
        Self {
            stats: WeaponStats {
                weapon_type: WeaponType::Ranged,
                base_damage: 20,
                attack_cooldown: 1.0,
                cooldown_timer: 0.0,
                attack_radius: 0.0,
                windup_duration: 0.0,
                attack_duration: 0.0,
                recovery_duration: 0.0,
                parry_window: 0.0,
                parry_active_duration: 0.0,
                stagger_duration: 0.0,
                range: 50.0,
                projectile_speed: 500.0,
                hearing_range: 200.0,
            },
        }
    }
}

// ============================================================================
// ArmorStatsTemplate
// ============================================================================

/// Armor stats template
#[derive(Clone, Debug, Reflect)]
pub struct ArmorStatsTemplate {
    /// Defense rating (damage reduction)
    pub defense: u32,
    /// Consumable slot bonus (0-3 доп слота)
    pub consumable_slot_bonus: u8,
}

// ============================================================================
// ConsumableEffect
// ============================================================================

/// Consumable effect (применяется при use)
#[derive(Clone, Debug, Reflect)]
pub enum ConsumableEffect {
    /// Восстановить HP
    RestoreHealth { amount: u32 },
    /// Восстановить stamina
    RestoreStamina { amount: u32 },
    /// Spawn projectile (grenade)
    SpawnProjectile { prefab_path: String, damage: u32 },
}

// ============================================================================
// ItemInstance (runtime данные)
// ============================================================================

/// Runtime item instance (конкретный предмет)
///
/// Хранится в `Inventory`, `EquippedWeapons`, `ConsumableSlots`.
/// Mutable state (durability, ammo, stack size).
#[derive(Clone, Debug, Reflect)]
pub struct ItemInstance {
    /// Ссылка на definition
    pub definition_id: ItemId,
    /// Stack size (для consumables/materials)
    pub stack_size: u32,
    /// Durability (0.0-1.0 для weapons/armor)
    pub durability: Option<f32>,
    /// Ammo count (для ranged weapons)
    pub ammo_count: Option<u32>,
}

impl ItemInstance {
    /// Создать новый item instance
    pub fn new(definition_id: impl Into<ItemId>) -> Self {
        Self {
            definition_id: definition_id.into(),
            stack_size: 1,
            durability: Some(1.0), // Полная прочность
            ammo_count: None,
        }
    }

    /// Создать weapon instance с ammo
    pub fn weapon_with_ammo(definition_id: impl Into<ItemId>, ammo: u32) -> Self {
        Self {
            definition_id: definition_id.into(),
            stack_size: 1,
            durability: Some(1.0),
            ammo_count: Some(ammo),
        }
    }

    /// Создать consumable stack
    pub fn consumable_stack(definition_id: impl Into<ItemId>, count: u32) -> Self {
        Self {
            definition_id: definition_id.into(),
            stack_size: count,
            durability: None,
            ammo_count: None,
        }
    }
}

// ============================================================================
// ItemDefinitions (Resource)
// ============================================================================

/// Item definitions lookup table (resource)
///
/// Хранит все статические данные предметов.
/// Создаётся один раз при запуске игры (hardcoded или из RON).
#[derive(Resource, Clone, Debug)]
pub struct ItemDefinitions {
    definitions: HashMap<ItemId, ItemDefinition>,
}

impl ItemDefinitions {
    /// Создать пустой registry
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
        }
    }

    /// Получить definition по ID
    pub fn get(&self, id: &ItemId) -> Option<&ItemDefinition> {
        self.definitions.get(id)
    }

    /// Добавить definition
    pub fn add(&mut self, definition: ItemDefinition) {
        self.definitions.insert(definition.id.clone(), definition);
    }

    /// Получить все IDs
    pub fn all_ids(&self) -> Vec<&ItemId> {
        self.definitions.keys().collect()
    }
}

impl Default for ItemDefinitions {
    /// Hardcoded definitions (базовые items)
    fn default() -> Self {
        let mut defs = Self::new();

        // === WEAPONS ===

        // Melee sword (large)
        defs.add(ItemDefinition {
            id: "melee_sword".into(),
            name: "Combat Sword".to_string(),
            item_type: ItemType::Weapon {
                size: WeaponSize::Large,
            },
            weapon_template: Some(WeaponStatsTemplate::melee_sword()),
            prefab_path: Some("res://actors/test_sword.tscn".to_string()),
            attachment_point: Some("%RightHandAttachment".to_string()),
            armor_stats: None,
            consumable_effect: None,
        });

        // Dagger (small)
        defs.add(ItemDefinition {
            id: "dagger".into(),
            name: "Combat Dagger".to_string(),
            item_type: ItemType::Weapon {
                size: WeaponSize::Small,
            },
            weapon_template: Some(WeaponStatsTemplate::dagger()),
            prefab_path: Some("res://actors/test_sword.tscn".to_string()), // Временно используем sword model
            attachment_point: Some("%RightHandAttachment".to_string()),
            armor_stats: None,
            consumable_effect: None,
        });

        // Pistol (small)
        defs.add(ItemDefinition {
            id: "pistol_basic".into(),
            name: "Basic Pistol".to_string(),
            item_type: ItemType::Weapon {
                size: WeaponSize::Small,
            },
            weapon_template: Some(WeaponStatsTemplate::ranged_pistol()),
            prefab_path: Some("res://actors/test_pistol.tscn".to_string()),
            attachment_point: Some("%RightHandAttachment".to_string()),
            armor_stats: None,
            consumable_effect: None,
        });

        // Rifle (large)
        defs.add(ItemDefinition {
            id: "rifle_basic".into(),
            name: "Basic Rifle".to_string(),
            item_type: ItemType::Weapon {
                size: WeaponSize::Large,
            },
            weapon_template: Some(WeaponStatsTemplate::ranged_rifle()),
            prefab_path: Some("res://actors/test_pistol.tscn".to_string()), // Временно используем pistol model
            attachment_point: Some("%RightHandAttachment".to_string()),
            armor_stats: None,
            consumable_effect: None,
        });

        // === ARMOR ===

        // Military armor (лучшая броня)
        defs.add(ItemDefinition {
            id: "armor_military".into(),
            name: "Military Combat Armor".to_string(),
            item_type: ItemType::Armor,
            weapon_template: None,
            prefab_path: None, // TODO: armor prefab
            attachment_point: Some("%Body".to_string()),
            armor_stats: Some(ArmorStatsTemplate {
                defense: 50,
                consumable_slot_bonus: 3, // Unlock все 5 слотов (2 базовых + 3 бонуса)
            }),
            consumable_effect: None,
        });

        // Tactical armor (средняя броня)
        defs.add(ItemDefinition {
            id: "armor_tactical".into(),
            name: "Tactical Vest".to_string(),
            item_type: ItemType::Armor,
            weapon_template: None,
            prefab_path: None, // TODO: armor prefab
            attachment_point: Some("%Body".to_string()),
            armor_stats: Some(ArmorStatsTemplate {
                defense: 30,
                consumable_slot_bonus: 2, // Unlock 4 слота (2 + 2)
            }),
            consumable_effect: None,
        });

        // Light armor (лёгкая броня)
        defs.add(ItemDefinition {
            id: "armor_light".into(),
            name: "Light Armor".to_string(),
            item_type: ItemType::Armor,
            weapon_template: None,
            prefab_path: None, // TODO: armor prefab
            attachment_point: Some("%Body".to_string()),
            armor_stats: Some(ArmorStatsTemplate {
                defense: 15,
                consumable_slot_bonus: 1, // Unlock 3 слота (2 + 1)
            }),
            consumable_effect: None,
        });

        // Scrap armor (самая слабая броня)
        defs.add(ItemDefinition {
            id: "armor_scrap".into(),
            name: "Scrap Armor".to_string(),
            item_type: ItemType::Armor,
            weapon_template: None,
            prefab_path: None, // TODO: armor prefab
            attachment_point: Some("%Body".to_string()),
            armor_stats: Some(ArmorStatsTemplate {
                defense: 5,
                consumable_slot_bonus: 0, // Только базовые 2 слота
            }),
            consumable_effect: None,
        });

        // === CONSUMABLES ===

        // Health kit
        defs.add(ItemDefinition {
            id: "health_kit".into(),
            name: "Health Kit".to_string(),
            item_type: ItemType::Consumable,
            weapon_template: None,
            prefab_path: None,
            attachment_point: None,
            armor_stats: None,
            consumable_effect: Some(ConsumableEffect::RestoreHealth { amount: 50 }),
        });

        // Stamina boost
        defs.add(ItemDefinition {
            id: "stamina_boost".into(),
            name: "Stamina Boost".to_string(),
            item_type: ItemType::Consumable,
            weapon_template: None,
            prefab_path: None,
            attachment_point: None,
            armor_stats: None,
            consumable_effect: Some(ConsumableEffect::RestoreStamina { amount: 100 }),
        });

        // Frag grenade
        defs.add(ItemDefinition {
            id: "grenade_frag".into(),
            name: "Frag Grenade".to_string(),
            item_type: ItemType::Consumable,
            weapon_template: None,
            prefab_path: None,
            attachment_point: None,
            armor_stats: None,
            consumable_effect: Some(ConsumableEffect::SpawnProjectile {
                prefab_path: "res://actors/test_projectile.tscn".to_string(),
                damage: 75,
            }),
        });

        defs
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_definitions_default() {
        let defs = ItemDefinitions::default();

        // Weapons
        assert!(defs.get(&"melee_sword".into()).is_some());
        assert!(defs.get(&"pistol_basic".into()).is_some());
        assert!(defs.get(&"rifle_basic".into()).is_some());
        assert!(defs.get(&"dagger".into()).is_some());

        // Armor
        assert!(defs.get(&"armor_military".into()).is_some());
        assert!(defs.get(&"armor_tactical".into()).is_some());
        assert!(defs.get(&"armor_light".into()).is_some());
        assert!(defs.get(&"armor_scrap".into()).is_some());

        // Consumables
        assert!(defs.get(&"health_kit".into()).is_some());
        assert!(defs.get(&"stamina_boost".into()).is_some());
        assert!(defs.get(&"grenade_frag".into()).is_some());
    }

    #[test]
    fn test_weapon_template_to_stats() {
        let template = WeaponStatsTemplate::melee_sword();
        let stats = template.to_weapon_stats();

        assert_eq!(stats.base_damage, 25);
        assert_eq!(stats.attack_cooldown, 1.0);
        assert_eq!(stats.cooldown_timer, 0.0);
        assert!(stats.is_melee());
    }

    #[test]
    fn test_item_instance_new() {
        let item = ItemInstance::new("melee_sword");
        assert_eq!(item.definition_id, "melee_sword".into());
        assert_eq!(item.stack_size, 1);
        assert_eq!(item.durability, Some(1.0));
        assert_eq!(item.ammo_count, None);
    }

    #[test]
    fn test_item_instance_consumable_stack() {
        let item = ItemInstance::consumable_stack("health_kit", 5);
        assert_eq!(item.definition_id, "health_kit".into());
        assert_eq!(item.stack_size, 5);
        assert_eq!(item.durability, None);
    }
}
