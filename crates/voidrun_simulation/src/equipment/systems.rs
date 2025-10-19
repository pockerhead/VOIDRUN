//! Equipment system implementations
//!
//! # Systems
//!
//! **Weapon lifecycle:**
//! - `process_equip_weapon` — equip weapon в слот
//! - `process_unequip_weapon` — unequip weapon из слота
//! - `process_weapon_swap` — smooth swap активного оружия
//!
//! **Armor lifecycle:**
//! - `process_equip_armor` — equip armor
//! - `process_unequip_armor` — unequip armor
//!
//! **Consumables:**
//! - `process_use_consumable` — use consumable из слота

use bevy::prelude::*;
use crate::{
    components::equipment::*,
    equipment::events::*,
    item_system::{ItemDefinitions, ItemInstance},
    log, log_error,
    Attachment, AttachmentType, WeaponStats,
};

// ============================================================================
// Weapon Equip
// ============================================================================

/// Process equip weapon intents
pub fn process_equip_weapon(
    mut commands: Commands,
    mut events: EventReader<EquipWeaponIntent>,
    mut equipped: Query<(&mut EquippedWeapons, Option<&mut Inventory>)>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok((mut weapons, mut inventory)) = equipped.get_mut(intent.entity) else {
            log_error(&format!("Entity {:?} missing EquippedWeapons", intent.entity));
            continue;
        };

        let slot_index = intent.slot.to_index();

        // 1. Unequip старое оружие (если есть)
        if let Some(old_item) = weapons.get_slot_mut(slot_index).take() {
            // Вернуть в inventory
            if let Some(ref mut inv) = inventory {
                inv.add_item(ItemInstance {
                    definition_id: old_item.definition_id.clone(),
                    stack_size: 1,
                    durability: Some(old_item.durability),
                    ammo_count: old_item.ammo_count,
                });
            }

            // Если это активный слот → удалить WeaponStats + Attachment
            if weapons.active_slot == slot_index {
                commands.entity(intent.entity)
                    .remove::<WeaponStats>()
                    .remove::<Attachment>();
            }
        }

        // 2. Equip новое оружие
        let Some(def) = definitions.get(&intent.item.definition_id) else {
            log_error(&format!("ItemDefinition not found: {:?}", intent.item.definition_id));
            continue;
        };

        weapons.set_slot(slot_index, Some(EquippedItem {
            definition_id: intent.item.definition_id.clone(),
            durability: intent.item.durability.unwrap_or(1.0),
            ammo_count: intent.item.ammo_count,
        }));

        // 3. Если это активный слот → добавить WeaponStats + Attachment
        if weapons.active_slot == slot_index {
            let Some(template) = &def.weapon_template else {
                log_error("Item is not a weapon!");
                continue;
            };

            commands.entity(intent.entity).insert((
                template.to_weapon_stats(),
                Attachment {
                    prefab_path: def.prefab_path.clone().unwrap_or_default(),
                    attachment_point: def.attachment_point.clone().unwrap_or_default(),
                    attachment_type: AttachmentType::Weapon,
                },
            ));

            log(&format!("✅ Equipped weapon {} to slot {:?}", def.name, intent.slot));
        }
    }
}

// ============================================================================
// Weapon Unequip
// ============================================================================

/// Process unequip weapon intents
pub fn process_unequip_weapon(
    mut commands: Commands,
    mut events: EventReader<UnequipWeaponIntent>,
    mut equipped: Query<(&mut EquippedWeapons, Option<&mut Inventory>)>,
) {
    for intent in events.read() {
        let Ok((mut weapons, mut inventory)) = equipped.get_mut(intent.entity) else {
            continue;
        };

        let slot_index = intent.slot.to_index();

        // 1. Take weapon из слота
        let Some(old_item) = weapons.get_slot_mut(slot_index).take() else {
            log_error(&format!("Slot {:?} already empty", intent.slot));
            continue;
        };

        // 2. Вернуть в inventory
        if let Some(ref mut inv) = inventory {
            inv.add_item(ItemInstance {
                definition_id: old_item.definition_id.clone(),
                stack_size: 1,
                durability: Some(old_item.durability),
                ammo_count: old_item.ammo_count,
            });
        }

        // 3. Если это активный слот → удалить WeaponStats + Attachment
        if weapons.active_slot == slot_index {
            commands.entity(intent.entity)
                .remove::<WeaponStats>()
                .remove::<Attachment>();

            log(&format!("🗑️ Unequipped weapon from slot {:?}", intent.slot));
        }
    }
}

// ============================================================================
// Weapon Swap
// ============================================================================

/// Process weapon swap intents (smooth transition)
pub fn process_weapon_swap(
    mut commands: Commands,
    mut events: EventReader<SwapActiveWeaponIntent>,
    mut equipped: Query<&mut EquippedWeapons>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok(mut weapons) = equipped.get_mut(intent.entity) else {
            continue;
        };

        // Guard: уже активен
        if weapons.active_slot == intent.target_slot {
            continue;
        }

        // Guard: слот пустой
        let Some(new_weapon) = weapons.get_slot(intent.target_slot) else {
            log_error(&format!("⚠️ Slot {} пустой", intent.target_slot));
            continue;
        };

        let Some(def) = definitions.get(&new_weapon.definition_id) else {
            continue;
        };

        // === Smooth swap flow ===

        // 1. Update active slot
        weapons.active_slot = intent.target_slot;

        // 2. Update WeaponStats + Attachment
        // NOTE: attach_prefabs_main_thread автоматически detach старый prefab при Changed<Attachment>
        let Some(template) = &def.weapon_template else {
            continue;
        };

        commands.entity(intent.entity).insert((
            template.to_weapon_stats(),
            Attachment {
                prefab_path: def.prefab_path.clone().unwrap_or_default(),
                attachment_point: def.attachment_point.clone().unwrap_or_default(),
                attachment_type: AttachmentType::Weapon,
            },
        ));

        log(&format!("✅ Weapon swap → slot {} ({}, {})",
            intent.target_slot,
            def.name,
            if template.stats.is_melee() { "melee" } else { "ranged" }
        ));
    }
}

// ============================================================================
// Armor Equip
// ============================================================================

/// Process equip armor intents
pub fn process_equip_armor(
    mut commands: Commands,
    mut events: EventReader<EquipArmorIntent>,
    mut consumables: Query<&mut ConsumableSlots>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Some(def) = definitions.get(&intent.item.definition_id) else {
            continue;
        };

        let Some(armor_stats) = &def.armor_stats else {
            log_error("Item is not armor!");
            continue;
        };

        // 1. Add Armor component
        commands.entity(intent.entity).insert(Armor {
            definition_id: intent.item.definition_id.clone(),
            durability: intent.item.durability.unwrap_or(1.0),
            defense: armor_stats.defense,
            consumable_slot_bonus: armor_stats.consumable_slot_bonus,
        });

        // 2. Add Attachment (визуал)
        if let Some(prefab_path) = &def.prefab_path {
            commands.entity(intent.entity).insert(Attachment {
                prefab_path: prefab_path.clone(),
                attachment_point: "%Body".to_string(),
                attachment_type: AttachmentType::Armor,
            });
        }

        // 3. Unlock consumable slots
        if let Ok(mut slots) = consumables.get_mut(intent.entity) {
            let unlocked = 2 + armor_stats.consumable_slot_bonus;
            slots.unlock_slots(unlocked);

            log(&format!("✅ Armor equipped - {} consumable slots unlocked", unlocked));
        }
    }
}

// ============================================================================
// Armor Unequip
// ============================================================================

/// Process unequip armor intents
pub fn process_unequip_armor(
    mut commands: Commands,
    mut events: EventReader<UnequipArmorIntent>,
    mut consumables: Query<&mut ConsumableSlots>,
) {
    for intent in events.read() {
        // 1. Remove Armor component
        commands.entity(intent.entity).remove::<Armor>();

        // 2. Remove Attachment (визуал)
        // NOTE: Attachment для armor может быть shared с другими items
        // Поэтому удаляем только если attachment_type == Armor
        // TODO: Implement proper multi-attachment tracking

        // 3. Lock consumable slots (обратно к базовым 2)
        if let Ok(mut slots) = consumables.get_mut(intent.entity) {
            slots.unlock_slots(2);
            log("🗑️ Armor unequipped - consumable slots locked to 2");
        }
    }
}

// ============================================================================
// Consumable Use
// ============================================================================

/// Process use consumable intents
pub fn process_use_consumable(
    mut events: EventReader<UseConsumableIntent>,
    mut consumables: Query<&mut ConsumableSlots>,
    mut health: Query<&mut crate::components::actor::Health>,
    mut stamina: Query<&mut crate::components::actor::Stamina>,
    definitions: Res<ItemDefinitions>,
) {
    for intent in events.read() {
        let Ok(mut slots) = consumables.get_mut(intent.entity) else {
            continue;
        };

        // Guard: слот разблокирован?
        if !slots.is_slot_unlocked(intent.slot_index) {
            log_error("⚠️ Слот заблокирован - нужна лучшая броня!");
            continue;
        }

        // Take consumable из слота
        let Some(item) = slots.take_slot(intent.slot_index) else {
            log_error("⚠️ Слот пустой");
            continue;
        };

        // Get definition
        let Some(def) = definitions.get(&item.definition_id) else {
            continue;
        };

        // Apply consumable effect
        let Some(effect) = &def.consumable_effect else {
            continue;
        };

        match effect {
            crate::item_system::ConsumableEffect::RestoreHealth { amount } => {
                if let Ok(mut hp) = health.get_mut(intent.entity) {
                    hp.current = (hp.current + *amount).min(hp.max);
                    log(&format!("✅ Использован {} (+{} HP)", def.name, amount));
                }
            }
            crate::item_system::ConsumableEffect::RestoreStamina { amount } => {
                if let Ok(mut stam) = stamina.get_mut(intent.entity) {
                    stam.current = (stam.current + *amount as f32).min(stam.max);
                    log(&format!("✅ Использован {} (+{} stamina)", def.name, amount));
                }
            }
            crate::item_system::ConsumableEffect::SpawnProjectile { .. } => {
                // TODO: Implement grenade spawn (Phase 5)
                log(&format!("✅ Использован {} (grenade)", def.name));
            }
        }
    }
}
