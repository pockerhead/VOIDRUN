//! Мост между Godot и Rust ECS симуляцией (100% Rust, no GDScript)
//!
//! Архитектура:
//! - Создаёт всю 3D сцену программно в ready()
//! - Каждый frame: ECS update → sync transforms → update health bars

mod effects;
mod logger;
mod scene;
mod spawn;
mod systems_setup;

use crate::systems::{AttachmentRegistry, SceneRoot, VisualRegistry, VisionTracking};
use godot::classes::{INode3D, Node};
use godot::prelude::*;
use logger::GodotLogger;
use spawn::{spawn_melee_npc, spawn_test_npc};
use voidrun_simulation::{create_headless_app, LogLevel, SimulationPlugin};

/// SimulationBridge: главный node для Godot ↔ ECS интеграции
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SimulationBridge {
    base: Base<Node3D>,

    /// Bevy ECS App (симуляция + NonSend visual registries)
    simulation: Option<bevy::app::App>,
}

#[godot_api]
impl INode3D for SimulationBridge {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            simulation: None,
        }
    }

    fn ready(&mut self) {
        GodotLogger::clear_log_file();
        voidrun_simulation::set_logger(Box::new(GodotLogger));
        voidrun_simulation::set_log_level(LogLevel::Debug);
        voidrun_simulation::log("SimulationBridge ready - building 3D scene in Rust");

        // 1. Создаём navigation region + ground
        self.create_navigation_region();

        // 2. Создаём lights
        self.create_lights();

        // 3. Создаём camera
        self.create_camera();

        // 3.5 Создаём DebugOverlay UI (FPS counter, spawn buttons)
        self.create_debug_overlay();

        // 4. Инициализируем ECS симуляцию
        let mut app = create_headless_app(42);
        app.add_plugins(SimulationPlugin);

        // 4.1 Регистрируем NonSend resources (main thread only)
        app.insert_non_send_resource(VisualRegistry::default());
        app.insert_non_send_resource(AttachmentRegistry::default());
        app.insert_non_send_resource(VisionTracking::default());
        app.insert_non_send_resource(crate::projectile_registry::GodotProjectileRegistry::default());
        app.insert_non_send_resource(SceneRoot {
            node: self.base().clone().upcast::<Node3D>(),
        });

        // 4.3 Регистрируем custom schedules + timer systems
        systems_setup::register_schedules(&mut app);

        // 4.4 Регистрируем все ECS systems
        systems_setup::register_systems(&mut app);

        self.simulation = Some(app);

        voidrun_simulation::log("Scene ready: Press 'Spawn NPCs' button to spawn test NPCs");
    }

    fn process(&mut self, delta: f64) {
        // Обновляем симуляцию
        if let Some(app) = &mut self.simulation {
            // Передаём delta time в Bevy (для movement system)
            app.world_mut()
                .insert_resource(crate::systems::GodotDeltaTime(delta as f32));

            app.update(); // ECS systems выполнятся, включая attach/detach_prefabs_main_thread
        }

        // Обрабатываем hit effects (DamageDealt события)
        self.process_hit_effects();
    }
}

#[godot_api]
impl SimulationBridge {
    /// Spawn NPCs button callback (вызывается при нажатии кнопки)
    #[func]
    pub fn spawn_npcs(&mut self) {
        voidrun_simulation::log("🎮 Spawn button pressed - spawning test NPCs");

        let Some(app) = &mut self.simulation else {
            voidrun_simulation::log_error("❌ Simulation not initialized!");
            return;
        };

        // Спавним NPC через Commands
        let world = app.world_mut();
        let mut commands = world.commands();

        // spawn_melee_npc(&mut commands, (0.0, 0.0, 3.0), 1, 60);
        spawn_test_npc(&mut commands, (25.0, 0.0, 6.0), 1, 60);
        // spawn_melee_npc(&mut commands, (21.0, 0.0, 6.0), 1, 60);

        // spawn_melee_npc(&mut commands, (0.0, 0.0, 0.0), 2, 60);
        spawn_test_npc(&mut commands, (-26.0, 0.0, -5.0), 2, 60);
        // spawn_melee_npc(&mut commands, (-16.0, 0.0, -6.0), 2, 60);

        // spawn_melee_npc(&mut commands, (3.0, 0.0, -6.0), 3, 60);
        spawn_test_npc(&mut commands, (2.0, 0.0, -5.0), 3, 60);
        // spawn_melee_npc(&mut commands, (1.0, 0.0, -6.0), 3, 60);

        voidrun_simulation::log("✅ NPCs spawned successfully (9 NPCs, 3 factions)");
    }

    /// Spawn player button callback (вызывается при нажатии кнопки)
    #[func]
    pub fn spawn_player(&mut self) {
        voidrun_simulation::log("🎮 Spawn Player button pressed");

        let Some(app) = &mut self.simulation else {
            voidrun_simulation::log_error("❌ Simulation not initialized!");
            return;
        };

        // Spawn player entity через helper
        let player_entity = {
            let world = app.world_mut();
            let mut entity_commands = world.spawn_empty();
            let player_entity = entity_commands.id();

            // Используем spawn напрямую вместо Commands
            entity_commands.insert((
                voidrun_simulation::components::Player,
                voidrun_simulation::components::Actor { faction_id: 1 },
                voidrun_simulation::StrategicPosition::from_world_position(
                    bevy::prelude::Vec3::new(0.0, 2.0, 0.0),
                ),
                voidrun_simulation::PrefabPath::new("res://actors/test_player.tscn"),
                voidrun_simulation::Health {
                    current: 100,
                    max: 100,
                },
                voidrun_simulation::Stamina {
                    current: 100.0,
                    max: 100.0,
                    regen_rate: 10.0,
                },
                voidrun_simulation::WeaponStats::melee_sword(),
                voidrun_simulation::Attachment {
                    prefab_path: "res://actors/test_sword.tscn".to_string(),
                    attachment_point: "%RightHandAttachment".to_string(),
                    attachment_type: voidrun_simulation::AttachmentType::Weapon,
                },
                // Equipment components (new system)
                voidrun_simulation::EquippedWeapons {
                    primary_large_1: Some(voidrun_simulation::EquippedItem {
                        definition_id: "melee_sword".into(),
                        durability: 1.0,
                        ammo_count: None,
                    }),
                    primary_large_2: None,
                    secondary_small_1: Some(voidrun_simulation::EquippedItem {
                        definition_id: "pistol_basic".into(),
                        durability: 1.0,
                        ammo_count: Some(30),
                    }),
                    secondary_small_2: None,
                    active_slot: 0, // Активен slot 0 (меч)
                },
                voidrun_simulation::ConsumableSlots::default(), // Базовые 2 слота
                voidrun_simulation::Inventory::empty(), // Пустой инвентарь пока
                // Player shooting components
                voidrun_simulation::components::player_shooting::AimMode::default(), // Hip Fire по умолчанию
            ));

            player_entity
        };

        // Создаём PlayerInputController node и setup simulation_bridge_path
        let mut controller = godot::prelude::Gd::<crate::input::PlayerInputController>::from_init_fn(
            |base| crate::input::PlayerInputController::init(base),
        );

        // Set simulation_bridge_path (абсолютный путь к SimulationBridge)
        let bridge_path = self.base().get_path();
        controller.bind_mut().simulation_bridge_path = bridge_path.into();

        // Добавляем PlayerInputController как child node SimulationBridge
        self.base_mut().add_child(&controller.upcast::<Node>());

        voidrun_simulation::log(&format!(
            "✅ Player spawned successfully (entity: {:?})",
            player_entity
        ));
    }

    /// Записать SafeVelocityComputed event в ECS (вызывается из AvoidanceReceiver)
    ///
    /// Flow:
    /// 1. NavigationAgent3D рассчитал safe_velocity с avoidance
    /// 2. Signal velocity_computed → AvoidanceReceiver::on_velocity_computed
    /// 3. AvoidanceReceiver вызывает этот метод
    /// 4. apply_safe_velocity_system читает event и применяет к CharacterBody3D
    pub fn write_safe_velocity_event(
        &mut self,
        entity: bevy::prelude::Entity,
        safe_velocity: bevy::prelude::Vec3,
        desired_velocity: bevy::prelude::Vec3,
    ) {
        let Some(app) = &mut self.simulation else {
            return;
        };

        app.world_mut()
            .send_event(crate::events::SafeVelocityComputed {
                entity,
                safe_velocity,
                desired_velocity,
            });
    }

    /// Emit PlayerInputEvent в ECS (вызывается из PlayerInputController)
    ///
    /// Flow:
    /// 1. PlayerInputController читает Godot Input (WASD, Space, LMB, RMB)
    /// 2. Вызывает этот метод каждый frame
    /// 3. Player input systems (process_player_input, player_combat_input) обрабатывают event
    pub fn emit_player_input_event(&mut self, input_event: crate::input::PlayerInputEvent) {
        let Some(app) = &mut self.simulation else {
            return;
        };

        app.world_mut().send_event(input_event);
    }

    /// Emit CameraToggleEvent в ECS (вызывается из PlayerInputController)
    ///
    /// Flow:
    /// 1. PlayerInputController читает [V] key press
    /// 2. Вызывает этот метод (debounced 300ms)
    /// 3. camera_toggle_system переключает FPS ↔ RTS camera
    pub fn emit_camera_toggle_event(&mut self, event: crate::input::CameraToggleEvent) {
        let Some(app) = &mut self.simulation else {
            return;
        };

        app.world_mut().send_event(event);
    }

    /// Emit MouseLookEvent в ECS (вызывается из PlayerInputController)
    ///
    /// Flow:
    /// 1. PlayerInputController читает mouse motion (unhandled_input)
    /// 2. Вызывает этот метод каждый mouse movement
    /// 3. player_mouse_look system вращает Actor body + CameraPivot
    pub fn emit_mouse_look_event(&mut self, event: crate::input::MouseLookEvent) {
        let Some(app) = &mut self.simulation else {
            return;
        };

        app.world_mut().send_event(event);
    }

    /// Emit WeaponSwitchEvent в ECS (вызывается из PlayerInputController)
    ///
    /// Flow:
    /// 1. PlayerInputController читает Digit1-9 key press
    /// 2. Вызывает этот метод с slot_index (0-8)
    /// 3. process_player_weapon_switch конвертирует в WeaponSwitchIntent
    /// 4. process_weapon_switch меняет ActiveWeaponSlot + Attachment
    pub fn emit_weapon_switch_event(&mut self, event: crate::input::WeaponSwitchEvent) {
        let Some(app) = &mut self.simulation else {
            return;
        };

        app.world_mut().send_event(event);
    }
}
