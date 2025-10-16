//! Player control marker component
//!
//! Отмечает entity которым управляет игрок через input (в отличие от AI).

use bevy::prelude::Component;

/// Marker component для player-controlled entity
///
/// Акторы БЕЗ этого компонента управляются AI systems.
/// Акторы С этим компонентом получают команды от player input systems.
///
/// # Архитектурная заметка
/// - AI systems используют `Without<Player>` filter (пропускают player-controlled акторов)
/// - Input systems используют `With<Player>` filter (только player-controlled акторы)
///
/// # Single-player
/// В single-player режиме обычно только один entity имеет этот компонент.
///
/// # Future: Possession
/// Для переключения контроля между акторами:
/// ```ignore
/// commands.entity(old_actor).remove::<Player>();
/// commands.entity(new_actor).insert(Player);
/// ```
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Player;
