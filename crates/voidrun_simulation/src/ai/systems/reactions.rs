//! AI reaction systems (death, damage, gunfire).

use bevy::prelude::*;
use crate::components::{Actor, MovementCommand};
use crate::ai::{AIState, SpottedEnemies, GodotAIEvent};

/// System: обработка смерти → переключение AI в Dead state
///
/// При HP == 0 отключаем AI (Dead state) чтобы мертвые не стреляли/двигались
pub fn handle_actor_death(
    mut actors: Query<(&crate::Health, &mut AIState), Changed<crate::Health>>,
) {
    for (health, mut state) in actors.iter_mut() {
        if health.current == 0 && !matches!(*state, AIState::Dead) {
            *state = AIState::Dead;
            crate::logger::log("Actor died → AI disabled (Dead state)");
        }
    }
}

/// System: AI реакция на полученный урон
///
/// Если актора ударили, он автоматически:
/// - Добавляет атакующего в SpottedEnemies (если враг)
/// - Разворачивается к атакующему (через MovementCommand::FollowEntity)
/// - FSM перейдёт в Combat на следующем тике через ai_fsm_transitions
///
/// Это обеспечивает естественную реакцию "ударили в спину → развернулся и дерёшься"
pub fn react_to_damage(
    mut damage_events: EventReader<crate::combat::DamageDealt>,
    mut actors: Query<(&Actor, &mut SpottedEnemies, &mut MovementCommand)>,
    attackers: Query<&Actor>,
) {
    for damage_event in damage_events.read() {
        // Получаем victim actor
        let Ok((victim_actor, mut spotted_enemies, mut command)) = actors.get_mut(damage_event.target) else {
            continue;
        };

        // Получаем attacker actor
        let Ok(attacker_actor) = attackers.get(damage_event.attacker) else {
            continue;
        };

        // Проверяем фракции: реагируем только на врагов
        if victim_actor.faction_id == attacker_actor.faction_id {
            // Friendly fire — игнорируем (или можно добавить другую логику)
            continue;
        }

        // Добавляем атакующего в SpottedEnemies (если ещё не там)
        if !spotted_enemies.enemies.contains(&damage_event.attacker) {
            spotted_enemies.enemies.push(damage_event.attacker);
            crate::logger::log(&format!(
                "⚠️ {:?} damaged by {:?} → added to SpottedEnemies",
                damage_event.target, damage_event.attacker
            ));
        }

        // Разворачиваемся к атакующему (FollowEntity даст NavigationAgent3D развернуться)
        *command = MovementCommand::FollowEntity {
            target: damage_event.attacker,
        };

        crate::logger::log(&format!(
            "🔥 {:?} hit by {:?} → following attacker",
            damage_event.target, damage_event.attacker
        ));
    }
}

/// System: AI реакция на звук выстрела
///
/// Архитектура:
/// - Слушает WeaponFired события (содержат shooter_position + hearing_range)
/// - Проверяет расстояние через StrategicPosition (chunk-aware distance)
/// - Генерирует ActorSpotted event для имитации "услышал стрелявшего"
/// - Устанавливает MovementCommand в сторону выстрела с разбросом 3м
///
/// Логика:
/// - Все актёры в радиусе слышат выстрел (союзники, враги, нейтралы)
/// - Skip: сам стрелявший, актёры уже в Combat (сосредоточены на своей цели)
/// - Радиус слышимости зависит от оружия (pistol ~25м, rifle ~40м, sniper ~60м)
pub fn ai_react_to_gunfire(
    mut gunfire_events: EventReader<crate::combat::WeaponFired>,
    mut actors: Query<(Entity, &Actor, &crate::StrategicPosition, &AIState, &mut MovementCommand)>,
    mut spotted_events: EventWriter<GodotAIEvent>,
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for fire_event in gunfire_events.read() {
        // Конвертируем world position → StrategicPosition для distance check
        let shooter_strategic = crate::StrategicPosition::from_world_position(
            fire_event.shooter_position
        );

        for (listener_entity, _listener_actor, listener_pos, ai_state, mut command) in actors.iter_mut() {
            // Skip: сам стрелявший
            if listener_entity == fire_event.shooter {
                continue;
            }

            // Skip: уже в Combat (сосредоточен на своей цели, не отвлекается)
            if matches!(ai_state, AIState::Combat { .. }) {
                continue;
            }

            // Проверка расстояния (chunk-aware distance через world positions)
            let listener_world_pos = listener_pos.to_world_position(0.5);
            let shooter_world_pos = shooter_strategic.to_world_position(0.5);
            let distance = listener_world_pos.distance(shooter_world_pos);

            if distance > fire_event.hearing_range {
                continue;
            }

            // ✅ Услышал выстрел!
            crate::logger::log(&format!(
                "🔊 Entity {:?} heard gunfire from {:?} at distance {:.1}m (range: {:.1}m)",
                listener_entity, fire_event.shooter, distance, fire_event.hearing_range
            ));

            // Генерируем ActorSpotted (имитация "услышал и заметил стрелявшего")
            spotted_events.write(GodotAIEvent::ActorSpotted {
                observer: listener_entity,
                target: fire_event.shooter,
            });

            // Идём в сторону выстрела с разбросом 3м (неуверенность в точной позиции)
            let random_offset = Vec3::new(
                rng.gen_range(-1.0..1.0), // -1..1
                0.0,
                rng.gen_range(-1.0..1.0),
            ) * 3.0; // 3м разброс

            let investigate_pos = fire_event.shooter_position + random_offset;
            *command = MovementCommand::MoveToPosition {
                target: investigate_pos,
            };

            crate::logger::log(&format!(
                "  → Entity {:?} moving to investigate gunfire at {:?}",
                listener_entity, investigate_pos
            ));
        }
    }
}
