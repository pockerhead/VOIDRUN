//! Attachment компоненты: динамические префабы (weapons, items, modules)

use bevy::prelude::*;

/// Attachment — привязка TSCN prefab к attachment point
///
/// Используется для weapons, items, ship modules, vehicle accessories.
/// Архитектура: ADR-007 (TSCN Prefabs + Dynamic Attachment)
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Attachment {
    /// Путь к TSCN prefab (например "res://actors/test_pistol.tscn")
    pub prefab_path: String,

    /// Attachment point на host prefab (например "RightHand/WeaponAttachment")
    pub attachment_point: String,

    /// Тип attachment (для logic/UI)
    pub attachment_type: AttachmentType,
}

impl Attachment {
    /// Создать attachment для weapon
    pub fn weapon(prefab_path: impl Into<String>) -> Self {
        Self {
            prefab_path: prefab_path.into(),
            attachment_point: "RightHand/WeaponAttachment".into(),
            attachment_type: AttachmentType::Weapon,
        }
    }

    /// Создать attachment для item (carried)
    pub fn item(prefab_path: impl Into<String>) -> Self {
        Self {
            prefab_path: prefab_path.into(),
            attachment_point: "RightHand/ItemAttachment".into(),
            attachment_type: AttachmentType::Item,
        }
    }
}

/// Attachment type (weapon, item, ship module, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum AttachmentType {
    Weapon,
    Item,
}

/// Marker component: detach specific attachment
///
/// Система detach_prefabs_main_thread читает этот компонент → удаляет attachment → removes component.
/// Позволяет детально управлять detach (например убрать левую руку двуручного оружия, правую оставить).
/// Архитектура: ADR-007 (TSCN Prefabs + Dynamic Attachment)
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct DetachAttachment {
    /// Attachment point для detach (например "RightHand/WeaponAttachment")
    pub attachment_point: String,
}
