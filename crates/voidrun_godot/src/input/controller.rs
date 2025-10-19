//! PlayerInputController - Godot node для чтения player input
//!
//! Архитектура:
//! - Godot Node (child of SimulationBridge)
//! - Читает Input::singleton() каждый frame
//! - Emit PlayerInputEvent в ECS через SimulationBridge
//!
//! Flow:
//! 1. process() вызывается каждый frame
//! 2. Читаем WASD, Shift, Space, [V] через Input API
//! 3. unhandled_input() читает mouse motion для camera look
//! 4. ECS systems обрабатывают events

use godot::classes::{Input, InputEvent, InputEventMouseMotion, Node};
use godot::prelude::*;
use bevy::prelude::Vec2;

use super::events::{CameraToggleEvent, MouseLookEvent, PlayerInputEvent, WeaponSwitchEvent};

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

    /// Cooldown для [V] toggle (prevent spam)
    toggle_cooldown: f32,

    base: Base<Node>,
}

#[godot_api]
impl INode for PlayerInputController {
    fn init(base: Base<Node>) -> Self {
        Self {
            simulation_bridge_path: NodePath::from(""),
            toggle_cooldown: 0.0,
            base,
        }
    }

    fn ready(&mut self) {
        voidrun_simulation::log("PlayerInputController ready - waiting for player spawn");
    }

    fn process(&mut self, delta: f64) {
        // Update toggle cooldown
        self.toggle_cooldown = (self.toggle_cooldown - delta as f32).max(0.0);

        // Guard: SimulationBridge path не установлен (player ещё не spawned)
        if self.simulation_bridge_path.is_empty() {
            return;
        }

        // Читаем Input
        let input = Input::singleton();

        // [V] key - camera toggle (debounced) - используем action из input map
        if input.is_action_just_pressed("debug_toggle") && self.toggle_cooldown <= 0.0 {
            self.emit_camera_toggle_event();
            self.toggle_cooldown = 0.3; // 300ms cooldown
        }

        // Digit1-9 (slot1-9) + 0 (slot0) - weapon/consumable switch
        // Используем is_action_just_pressed для prevent repeated triggers
        if input.is_action_just_pressed("slot1") {
            self.emit_weapon_switch_event(0);
        } else if input.is_action_just_pressed("slot2") {
            self.emit_weapon_switch_event(1);
        } else if input.is_action_just_pressed("slot3") {
            self.emit_weapon_switch_event(2);
        } else if input.is_action_just_pressed("slot4") {
            self.emit_weapon_switch_event(3);
        } else if input.is_action_just_pressed("slot5") {
            self.emit_weapon_switch_event(4);
        } else if input.is_action_just_pressed("slot6") {
            self.emit_weapon_switch_event(5);
        } else if input.is_action_just_pressed("slot7") {
            self.emit_weapon_switch_event(6);
        } else if input.is_action_just_pressed("slot8") {
            self.emit_weapon_switch_event(7);
        } else if input.is_action_just_pressed("slot9") {
            self.emit_weapon_switch_event(8);
        } else if input.is_action_just_pressed("slot0") {
            self.emit_weapon_switch_event(9);
        }

        // WASD movement direction - get_vector (как в 3d-rpg player.gd)
        // Input.get_vector("move_left", "move_right", "move_forward", "move_back")
        let move_direction = input.get_vector(
            "input_left",
            "input_right",
            "input_forward",
            "input_backward",
        );

        // Sprint (Shift) - unlimited пока (используем is_action_pressed для continuous state)
        let sprint = input.is_action_pressed("input_sprint");

        // Jump (Space) - just_pressed через input map
        let jump = input.is_action_just_pressed("input_jump");

        // Attack (LMB) - just_pressed через input map
        let attack = input.is_action_just_pressed("input_attack");

        // Parry (RMB) - just_pressed через input map
        let parry = input.is_action_just_pressed("input_block");

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

    fn unhandled_input(&mut self, mut event: Gd<InputEvent>) {
        // Guard: SimulationBridge path не установлен
        if self.simulation_bridge_path.is_empty() {
            return;
        }

        // Mouse motion для camera look
        if let Ok(motion) = event.try_cast::<InputEventMouseMotion>() {
            let relative = motion.get_relative();

            // Emit MouseLookEvent
            self.emit_mouse_look_event(MouseLookEvent {
                delta_x: relative.x,
                delta_y: relative.y,
            });

            return;
        }

        // Consume player input actions чтобы UI не получал их
        // (предотвращает Space активацию UI buttons)
        let input = Input::singleton();
        if input.is_action_just_pressed("input_jump")
            || input.is_action_just_pressed("input_attack")
            || input.is_action_just_pressed("input_block")
            || input.is_action_just_pressed("debug_toggle")
            || input.is_action_pressed("input_forward")
            || input.is_action_pressed("input_backward")
            || input.is_action_pressed("input_left")
            || input.is_action_pressed("input_right")
            || input.is_action_pressed("input_sprint")
            || input.is_action_just_pressed("slot1")
            || input.is_action_just_pressed("slot2")
            || input.is_action_just_pressed("slot3")
            || input.is_action_just_pressed("slot4")
            || input.is_action_just_pressed("slot5")
            || input.is_action_just_pressed("slot6")
            || input.is_action_just_pressed("slot7")
            || input.is_action_just_pressed("slot8")
            || input.is_action_just_pressed("slot9")
            || input.is_action_just_pressed("slot0")
        {
            // Mark event as handled (UI не получит)
            self.base_mut()
                .get_viewport()
                .expect("viewport")
                .set_input_as_handled();
        }
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

    /// Emit CameraToggleEvent в ECS через SimulationBridge
    fn emit_camera_toggle_event(&mut self) {
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
            return;
        };

        bridge.bind_mut().emit_camera_toggle_event(CameraToggleEvent);
    }

    /// Emit MouseLookEvent в ECS через SimulationBridge
    fn emit_mouse_look_event(&mut self, event: MouseLookEvent) {
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
            return;
        };

        bridge.bind_mut().emit_mouse_look_event(event);
    }

    /// Emit WeaponSwitchEvent в ECS через SimulationBridge
    fn emit_weapon_switch_event(&mut self, slot_index: u8) {
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
            return;
        };

        bridge.bind_mut().emit_weapon_switch_event(WeaponSwitchEvent { slot_index });
    }
}
