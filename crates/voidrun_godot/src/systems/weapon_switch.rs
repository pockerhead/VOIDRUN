//! Weapon switch system — переключение оружия через hotbar
//!
//! # Flow
//!
//! 1. PlayerInputController (Godot) → WeaponSwitchEvent (Digit1-4 для weapons)
//! 2. `process_player_weapon_switch` → конвертирует в SwapActiveWeaponIntent (ECS)
//! 3. Equipment system (`process_weapon_swap`) → меняет active_slot + Attachment + WeaponStats
//! 4. `attach_prefabs_main_thread` → подхватит Changed<Attachment> и сменит визуал
//!
//! # Consumables
//! - Digit5-9 обрабатываются через `UseConsumableIntent` (Phase 5)

use bevy::prelude::*;
use voidrun_simulation::{Player, SwapActiveWeaponIntent};

use crate::input::WeaponSwitchEvent;

/// Process player weapon switch input (Godot → ECS)
///
/// Конвертирует WeaponSwitchEvent (Godot input) в SwapActiveWeaponIntent (ECS).
///
/// # Архитектура
/// - Читает: WeaponSwitchEvent (from PlayerInputController)
/// - Пишет: SwapActiveWeaponIntent
/// - Query: With<Player>
///
/// # Hotkeys
/// - Digit1-4 → weapon slots (handled here)
/// - Digit5-9 → consumable slots (handled in Phase 5)
pub fn process_player_weapon_switch(
    mut switch_events: EventReader<WeaponSwitchEvent>,
    mut intent_events: EventWriter<SwapActiveWeaponIntent>,
    player_query: Query<Entity, With<Player>>,
) {
    let Ok(player_entity) = player_query.single() else {
        return;
    };

    for event in switch_events.read() {
        // Guard: только weapon slots (0-3)
        if event.slot_index > 3 {
            // Consumables slots (4-8) обрабатываются в Phase 5
            continue;
        }

        // Generate SwapActiveWeaponIntent для player
        intent_events.send(SwapActiveWeaponIntent {
            entity: player_entity,
            target_slot: event.slot_index,
        });

        voidrun_simulation::log(&format!(
            "🔄 Player weapon swap request → slot {} (Digit{})",
            event.slot_index,
            event.slot_index + 1
        ));
    }
}
