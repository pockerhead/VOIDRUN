//! PlayerInputController - Godot node для чтения player input
//!
//! Архитектура:
//! - Godot Node (child of SimulationBridge)
//! - Читает Input::singleton() каждый frame
//! - Emit PlayerInputEvent в ECS через SimulationBridge
//!
//! Flow:
//! 1. process() вызывается каждый frame
//! 2. Читаем WASD, Shift, Space, LMB, RMB через Input API
//! 3. Emit PlayerInputEvent через SimulationBridge::emit_player_input_event()
//! 4. ECS systems обрабатывают event (process_player_input, player_combat_input)

use godot::classes::{Input, Node};
use godot::global::Key;
use godot::prelude::*;
use bevy::prelude::Vec2;

use super::events::PlayerInputEvent;

/// PlayerInputController - читает Godot Input и emit ECS events
///
/// # Setup
/// - Spawn как child node SimulationBridge
/// - Activated when player spawned
#[derive(GodotClass)]
#[class(base=Node)]
pub struct PlayerInputController {
    /// Путь к SimulationBridge (parent node, для emit events)
    /// Устанавливается при spawn player
    #[var]
    pub simulation_bridge_path: NodePath,

    base: Base<Node>,
}

#[godot_api]
impl INode for PlayerInputController {
    fn init(base: Base<Node>) -> Self {
        Self {
            simulation_bridge_path: NodePath::from(""),
            base,
        }
    }

    fn ready(&mut self) {
        voidrun_simulation::log("PlayerInputController ready - waiting for player spawn");
    }

    fn process(&mut self, _delta: f64) {
        // Guard: SimulationBridge path не установлен (player ещё не spawned)
        if self.simulation_bridge_path.is_empty() {
            return;
        }

        // Читаем Input
        let input = Input::singleton();

        // WASD movement direction
        let mut move_direction = Vector2::ZERO;

        if input.is_physical_key_pressed(Key::W) {
            move_direction.y -= 1.0; // Forward (Godot -Z convention)
        }
        if input.is_physical_key_pressed(Key::S) {
            move_direction.y += 1.0; // Backward (Godot +Z convention)
        }
        if input.is_physical_key_pressed(Key::A) {
            move_direction.x -= 1.0; // Left
        }
        if input.is_physical_key_pressed(Key::D) {
            move_direction.x += 1.0; // Right
        }

        // Normalize (diagonal movement не быстрее)
        if move_direction.length() > 0.0 {
            move_direction = move_direction.normalized();
        }

        // Sprint (Shift) - unlimited пока
        let sprint = input.is_physical_key_pressed(Key::SHIFT);

        // Jump (Space) - just_pressed
        let jump = input.is_key_pressed(Key::SPACE); // TODO: is_action_just_pressed когда setup input map

        // Attack (LMB) - just_pressed
        // TODO: mouse buttons когда setup input map
        let attack = false; // input.is_action_just_pressed("attack");

        // Parry (RMB) - just_pressed
        let parry = false; // input.is_action_just_pressed("parry");

        // Создаём PlayerInputEvent
        let input_event = PlayerInputEvent {
            move_direction: Vec2::new(move_direction.x, move_direction.y),
            sprint,
            jump,
            attack,
            parry,
        };

        // Emit event через SimulationBridge
        self.emit_player_input_event(input_event);
    }
}

impl PlayerInputController {
    /// Emit PlayerInputEvent в ECS через SimulationBridge
    ///
    /// Находит SimulationBridge через NodePath и вызывает метод для emit event
    fn emit_player_input_event(&mut self, input_event: PlayerInputEvent) {
        // Получить SimulationBridge через путь
        let Some(mut bridge) = self
            .base()
            .get_tree()
            .and_then(|tree| tree.get_root())
            .and_then(|root| {
                root.try_get_node_as::<crate::simulation_bridge::SimulationBridge>(
                    &self.simulation_bridge_path,
                )
            })
        else {
            voidrun_simulation::log_error(&format!(
                "PlayerInputController: SimulationBridge not found at path: {}",
                self.simulation_bridge_path
            ));
            return;
        };

        // Вызываем метод SimulationBridge для emit event
        bridge
            .bind_mut()
            .emit_player_input_event(input_event);
    }
}
