//! Debug overlay UI — FPS counter, spawn buttons, AI state logger
//!
//! Отдельный Godot node (Control) для debug информации.
//! Создаётся SimulationBridge в ready(), toggle с F3.

use godot::classes::{Button, Control, IControl, InputEvent, InputEventKey, Label};
use godot::global::Key;
use godot::prelude::*;

/// Debug overlay — UI panel с FPS counter, spawn buttons, debug info
///
/// # Функции
/// - FPS counter (обновляется каждые 0.2 сек)
/// - Spawn NPCs button (вызывает callback на SimulationBridge)
/// - Spawn Player button (вызывает callback на SimulationBridge)
/// - AI state debug logger (каждую секунду, если enabled)
/// - F3 toggle — показать/скрыть весь overlay
///
/// # Архитектура
/// - Создаётся SimulationBridge::ready()
/// - Хранит reference на SimulationBridge (для callback spawn_npcs/spawn_player)
/// - По умолчанию видим (visible = true), можно скрыть F3
#[derive(GodotClass)]
#[class(base=Control)]
pub struct DebugOverlay {
    base: Base<Control>,

    /// FPS label
    fps_label: Option<Gd<Label>>,

    /// Spawn NPCs button
    spawn_button: Option<Gd<Button>>,

    /// Spawn Player button
    player_button: Option<Gd<Button>>,

    /// FPS timer (для обновления каждые 0.2 сек)
    fps_timer: f32,

    /// Frame counter (для FPS calculation)
    frame_count: u32,

    /// Path к SimulationBridge (для вызова spawn методов)
    /// ВАЖНО: должен быть установлен ПЕРЕД добавлением в scene tree
    pub(crate) simulation_bridge_path: GString,
}

#[godot_api]
impl IControl for DebugOverlay {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            fps_label: None,
            spawn_button: None,
            player_button: None,
            fps_timer: 0.0,
            frame_count: 0,
            simulation_bridge_path: GString::from(""),
        }
    }

    fn ready(&mut self) {
        // Создаём UI elements
        self.create_ui();

        // Подключаем buttons если путь уже установлен
        if !self.simulation_bridge_path.is_empty() {
            self.connect_buttons();
        }

        voidrun_simulation::log("✅ DebugOverlay ready (F3 to toggle)");
    }

    fn process(&mut self, delta: f64) {
        // FPS counter update
        self.update_fps_counter(delta);
    }

    fn unhandled_key_input(&mut self, event: Gd<InputEvent>) {
        // F3 toggle
        let Some(key_event) = event.try_cast::<InputEventKey>().ok() else {
            return;
        };

        // Check if F3 pressed (just pressed, not held)
        if key_event.get_keycode() == Key::F3 && key_event.is_pressed() && !key_event.is_echo() {
            let is_visible = self.base().is_visible();
            self.base_mut().set_visible(!is_visible);

            let status = if !is_visible { "shown" } else { "hidden" };
            voidrun_simulation::log(&format!("🐛 Debug overlay {} (F3)", status));
        }
    }
}

#[godot_api]
impl DebugOverlay {
    /// Создать UI elements (FPS label, spawn buttons)
    fn create_ui(&mut self) {
        // === FPS Label (top-left) ===
        let mut fps_label = Label::new_alloc();
        fps_label.set_text("FPS: --");
        fps_label.set_position(Vector2::new(10.0, 10.0));

        // Font size
        fps_label.add_theme_font_size_override("font_size", 20);

        self.base_mut()
            .add_child(&fps_label.clone().upcast::<Node>());
        self.fps_label = Some(fps_label);

        // === Spawn NPCs Button (top-left, below FPS) ===
        let mut spawn_button = Button::new_alloc();
        spawn_button.set_text("Spawn NPCs");
        spawn_button.set_position(Vector2::new(10.0, 40.0));
        spawn_button.set_size(Vector2::new(150.0, 40.0));

        self.base_mut()
            .add_child(&spawn_button.clone().upcast::<Node>());
        self.spawn_button = Some(spawn_button);

        // === Spawn Player Button (top-left, below Spawn NPCs) ===
        let mut player_button = Button::new_alloc();
        player_button.set_text("Spawn Player");
        player_button.set_position(Vector2::new(10.0, 90.0));
        player_button.set_size(Vector2::new(150.0, 40.0));

        self.base_mut()
            .add_child(&player_button.clone().upcast::<Node>());
        self.player_button = Some(player_button);
    }

    /// Подключить button signals к SimulationBridge методам
    fn connect_buttons(&mut self) {
        if self.simulation_bridge_path.is_empty() {
            voidrun_simulation::log_error("❌ DebugOverlay: simulation_bridge_path not set!");
            return;
        }

        // Получаем SimulationBridge node
        let Some(bridge) = self
            .base()
            .try_get_node_as::<Node>(self.simulation_bridge_path.arg())
        else {
            voidrun_simulation::log_error(&format!(
                "❌ DebugOverlay: SimulationBridge not found at path: {}",
                self.simulation_bridge_path
            ));
            return;
        };

        // Spawn NPCs button → SimulationBridge::spawn_npcs()
        if let Some(mut button) = self.spawn_button.as_mut() {
            let callable = bridge.callable("spawn_npcs");
            button.connect("pressed", &callable);
        }

        // Spawn Player button → SimulationBridge::spawn_player()
        if let Some(mut button) = self.player_button.as_mut() {
            let callable = bridge.callable("spawn_player");
            button.connect("pressed", &callable);
        }

        voidrun_simulation::log("✅ DebugOverlay: buttons connected to SimulationBridge");
    }

    /// Update FPS counter (каждые 0.2 сек)
    fn update_fps_counter(&mut self, delta: f64) {
        self.fps_timer += delta as f32;
        self.frame_count += 1;

        // Update label каждые 0.2 сек
        if self.fps_timer >= 0.2 {
            let fps = self.frame_count as f32 / self.fps_timer;

            if let Some(mut label) = self.fps_label.as_mut() {
                label.set_text(&format!("FPS: {:.0}", fps));
            }

            // Reset counters
            self.fps_timer = 0.0;
            self.frame_count = 0;
        }
    }
}
