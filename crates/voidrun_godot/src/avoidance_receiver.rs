//! AvoidanceReceiver — wrapper node для NavigationAgent3D velocity_computed signal
//!
//! Архитектура:
//! - Godot Node (не Component!), добавляется в actor_node как child
//! - В _ready() подключается к NavigationAgent3D::velocity_computed signal
//! - В callback пишет Bevy Event: SafeVelocityComputed
//!
//! Flow:
//! 1. apply_navigation_velocity_main_thread вызывает nav_agent.set_velocity(desired)
//! 2. NavigationServer3D рассчитывает safe_velocity с avoidance
//! 3. Signal velocity_computed → on_velocity_computed callback
//! 4. Callback пишет SafeVelocityComputed event через SimulationBridge
//! 5. apply_safe_velocity_system читает event и применяет к CharacterBody3D
//!
//! КРИТИЧНО:
//! - Main thread only (Godot API)
//! - Entity ID хранится как i64 (Godot property)
//! - SimulationBridge path устанавливается при spawn (для EventWriter)

use godot::classes::{NavigationAgent3D, Node};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct AvoidanceReceiver {
    /// ECS Entity, которому принадлежит этот Godot node
    /// Хранится как i64 для совместимости с Godot properties
    #[var]
    pub entity_id: i64,

    /// Путь к SimulationBridge node (для доступа к World/EventWriter)
    /// Устанавливается при spawn в visual_sync.rs
    #[var]
    pub simulation_bridge_path: NodePath,

    /// Desired velocity (для debug логирования)
    /// Устанавливается apply_navigation_velocity_main_thread перед nav_agent.set_velocity()
    #[var]
    pub desired_velocity: Vector3,

    base: Base<Node>,
}

#[godot_api]
impl INode for AvoidanceReceiver {
    fn init(base: Base<Node>) -> Self {
        Self {
            entity_id: 0,
            simulation_bridge_path: NodePath::from(""),
            desired_velocity: Vector3::ZERO,
            base,
        }
    }

    fn ready(&mut self) {
        // Найти NavigationAgent3D sibling
        let Some(parent) = self.base().get_parent() else {
            voidrun_simulation::log_error("AvoidanceReceiver: no parent node");
            return;
        };

        let Some(mut nav_agent) = parent.try_get_node_as::<NavigationAgent3D>("NavigationAgent3D")
        else {
            voidrun_simulation::log_error("AvoidanceReceiver: NavigationAgent3D not found");
            return;
        };

        // Подключить signal velocity_computed(safe_velocity: Vector3)
        let callable = self.base().callable("on_velocity_computed");
        nav_agent.connect("velocity_computed", &callable);

        voidrun_simulation::log(&format!(
            "AvoidanceReceiver ready for entity {}, connected to velocity_computed signal",
            self.entity_id
        ));
    }
}

#[godot_api]
impl AvoidanceReceiver {
    /// Callback для NavigationAgent3D::velocity_computed(safe_velocity: Vector3)
    ///
    /// Вызывается NavigationServer3D когда рассчитан safe_velocity с avoidance.
    /// Записывает SafeVelocityComputed event через SimulationBridge.
    #[func]
    fn on_velocity_computed(&mut self, safe_velocity: Vector3) {
        // Получить SimulationBridge через путь (устанавливается в visual_sync.rs)
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
            voidrun_simulation::log_error(&format!("AvoidanceReceiver: SimulationBridge not found at path: {}", self.simulation_bridge_path));
            return;
        };

        // Debug logging: сравниваем desired vs safe velocity
        let velocity_diff = (safe_velocity - self.desired_velocity).length();

        // Записать SafeVelocityComputed event через SimulationBridge
        let entity = bevy::prelude::Entity::from_bits(self.entity_id as u64);
        bridge.bind_mut().write_safe_velocity_event(
            entity,
            bevy::prelude::Vec3::new(safe_velocity.x, safe_velocity.y, safe_velocity.z),
            bevy::prelude::Vec3::new(
                self.desired_velocity.x,
                self.desired_velocity.y,
                self.desired_velocity.z,
            ),
        );
    }
}
