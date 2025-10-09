//! GGRS Configuration для VOIDRUN
//!
//! Настройка P2P rollback netcode:
//! - 2-8 игроков P2P
//! - 64Hz tick rate (совпадает с FixedUpdate)
//! - Input encoding для combat (movement + attack button)
//! - Checksum validation для determinism

use bevy::prelude::*;
use bevy_ggrs::*;
use ggrs::{Config, InputStatus, PlayerHandle};

/// GGRS Config для VOIDRUN
///
/// Определяет типы для P2P session:
/// - Input: 64-bit encoded input (movement direction + buttons)
/// - State: u64 checksum для determinism validation
/// - Address: player network address
#[derive(Debug)]
pub struct VoidrunGGRSConfig;

impl Config for VoidrunGGRSConfig {
    /// Input — закодированный ввод игрока (64 bits)
    ///
    /// Битовая структура:
    /// - bits 0-15: direction X (f16 scaled to u16)
    /// - bits 16-31: direction Z (f16 scaled to u16)
    /// - bit 32: attack button
    /// - bit 33: block button (будущее)
    /// - bit 34: dodge button (будущее)
    /// - bits 35-63: reserved
    type Input = u64;

    /// State checksum для determinism validation
    type State = u64;

    /// Player network address (для P2P matchmaking)
    type Address = String;
}

/// Кодирует input в u64 для GGRS
///
/// Превращает MovementInput + buttons в компактный битовый формат.
pub fn encode_input(direction: Vec3, attack: bool, block: bool) -> u64 {
    let mut input: u64 = 0;

    // Direction X: scale [-1, 1] → [0, 65535]
    let x_scaled = ((direction.x + 1.0) * 32767.5).clamp(0.0, 65535.0) as u16;
    input |= (x_scaled as u64) << 0;

    // Direction Z: scale [-1, 1] → [0, 65535]
    let z_scaled = ((direction.z + 1.0) * 32767.5).clamp(0.0, 65535.0) as u16;
    input |= (z_scaled as u64) << 16;

    // Attack button
    if attack {
        input |= 1u64 << 32;
    }

    // Block button
    if block {
        input |= 1u64 << 33;
    }

    input
}

/// Декодирует u64 input обратно в структуру
///
/// Возвращает (direction, attack, block).
pub fn decode_input(input: u64) -> (Vec3, bool, bool) {
    // Direction X: scale [0, 65535] → [-1, 1]
    let x_scaled = (input & 0xFFFF) as u16;
    let x = (x_scaled as f32 / 32767.5) - 1.0;

    // Direction Z
    let z_scaled = ((input >> 16) & 0xFFFF) as u16;
    let z = (z_scaled as f32 / 32767.5) - 1.0;

    let direction = Vec3::new(x, 0.0, z);

    // Buttons
    let attack = (input & (1u64 << 32)) != 0;
    let block = (input & (1u64 << 33)) != 0;

    (direction, attack, block)
}

/// Система: читает GGRS inputs и применяет их к entities
///
/// Для каждого игрока берём input из GGRS и конвертируем в MovementInput + кнопки атаки.
pub fn read_ggrs_inputs(
    mut players: Query<(&PlayerHandle, &mut crate::physics::MovementInput)>,
    inputs: Res<PlayerInputs<VoidrunGGRSConfig>>,
) {
    for (player_handle, mut movement_input) in players.iter_mut() {
        let (input, status) = inputs[*player_handle];

        // Если input пришёл (не lag, не disconnect)
        if status == InputStatus::Confirmed {
            let (direction, _attack, _block) = decode_input(input);
            movement_input.direction = direction;

            // TODO: обработать attack/block buttons через события
        } else {
            // Lag/disconnect — используем prediction (last input)
            // Пока просто останавливаем движение
            movement_input.direction = Vec3::ZERO;
        }
    }
}

/// Создаёт checksum мира для determinism validation
///
/// Собирает состояние всех rollback entities и хеширует.
/// GGRS использует это для детекции desyncs.
pub fn create_world_checksum(world: &World) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Собираем все rollback entities в детерминированном порядке
    // (по Entity ID, отсортированные)
    let mut entities: Vec<Entity> = world
        .query_filtered::<Entity, With<crate::rollback::Rollback>>()
        .iter(world)
        .collect();
    entities.sort_by_key(|e| e.index());

    // Хешируем каждый rollback component
    for entity in entities {
        entity.index().hash(&mut hasher);

        // Transform
        if let Some(transform) = world.get::<Transform>(entity) {
            // Hash only relevant fields (translation + rotation)
            format!("{:.3}", transform.translation.x).hash(&mut hasher);
            format!("{:.3}", transform.translation.y).hash(&mut hasher);
            format!("{:.3}", transform.translation.z).hash(&mut hasher);
        }

        // Health
        if let Some(health) = world.get::<crate::components::Health>(entity) {
            health.current.hash(&mut hasher);
        }

        // Stamina (rounded для float determinism)
        if let Some(stamina) = world.get::<crate::components::Stamina>(entity) {
            format!("{:.2}", stamina.current).hash(&mut hasher);
        }

        // TODO: добавить остальные rollback components
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_input() {
        let direction = Vec3::new(0.5, 0.0, -0.8);
        let attack = true;
        let block = false;

        let encoded = encode_input(direction, attack, block);
        let (decoded_dir, decoded_attack, decoded_block) = decode_input(encoded);

        // Direction должен быть близок (float precision)
        assert!((decoded_dir.x - direction.x).abs() < 0.01);
        assert!((decoded_dir.z - direction.z).abs() < 0.01);

        // Buttons точные
        assert_eq!(decoded_attack, attack);
        assert_eq!(decoded_block, block);
    }

    #[test]
    fn test_encode_zero_input() {
        let encoded = encode_input(Vec3::ZERO, false, false);
        let (decoded_dir, attack, block) = decode_input(encoded);

        assert!((decoded_dir.x - 0.0).abs() < 0.01);
        assert!((decoded_dir.z - 0.0).abs() < 0.01);
        assert_eq!(attack, false);
        assert_eq!(block, false);
    }

    #[test]
    fn test_checksum_determinism() {
        use crate::*;

        let mut app1 = create_headless_app(42);
        app1.world_mut().spawn((
            Rollback,
            Transform::from_xyz(1.0, 2.0, 3.0),
            Actor { faction_id: 1 },
        ));

        let checksum1 = create_world_checksum(app1.world());

        let mut app2 = create_headless_app(42);
        app2.world_mut().spawn((
            Rollback,
            Transform::from_xyz(1.0, 2.0, 3.0),
            Actor { faction_id: 1 },
        ));

        let checksum2 = create_world_checksum(app2.world());

        assert_eq!(checksum1, checksum2, "Identical worlds should have identical checksums");
    }

    #[test]
    fn test_checksum_sensitivity() {
        use crate::*;

        let mut app1 = create_headless_app(42);
        app1.world_mut().spawn((
            Rollback,
            Transform::from_xyz(1.0, 2.0, 3.0),
            Actor { faction_id: 1 },
        ));

        let checksum1 = create_world_checksum(app1.world());

        // Изменяем позицию на 0.1m
        let mut app2 = create_headless_app(42);
        app2.world_mut().spawn((
            Rollback,
            Transform::from_xyz(1.1, 2.0, 3.0), // CHANGED
            Actor { faction_id: 1 },
        ));

        let checksum2 = create_world_checksum(app2.world());

        assert_ne!(checksum1, checksum2, "Different worlds should have different checksums");
    }
}
