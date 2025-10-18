//! Scene creation (navigation, lights, camera, UI)
//!
//! Extension методы для SimulationBridge (создание 3D сцены программно).

use super::SimulationBridge;
use crate::camera::rts_camera::RTSCamera3D;
use godot::classes::{light_3d::Param as LightParam, Button, CanvasLayer, DirectionalLight3D, Label, Node, Timer};
use godot::prelude::*;

impl SimulationBridge {
    /// Создать NavigationRegion3D + NavMesh (baking из SceneTree children)
    ///
    /// TEST: Проверяем что NavMesh запекается из StaticBody3D/CSGBox3D children,
    /// а не из процедурной геометрии (для будущего chunk building).
    pub(super) fn create_navigation_region(&mut self) {
        use crate::chunk_navmesh::{create_test_navigation_region_with_obstacles, NavMeshBakingParams};

        // 1. Создаём параметры NavMesh baking
        let mut params = NavMeshBakingParams::default();
        params.baking_aabb = godot::builtin::Aabb {
            position: Vector3::new(-200.0, -1.0, -200.0),
            size: Vector3::new(400.0, 10.0, 400.0), // Высота 10м (для боксов)
        };

        // 2. Создаём NavigationRegion3D с тестовыми obstacles через утилиту
        let mut nav_region = create_test_navigation_region_with_obstacles(&params);

        // 3. Добавляем NavigationRegion3D в сцену ПЕРЕД baking
        self.base_mut().add_child(&nav_region.clone().upcast::<Node>());

        voidrun_simulation::log("🔧 Baking NavMesh from SceneTree (StaticBody3D children)...");

        // 4. Bake NavMesh из SceneTree children (КРИТИЧНО: region должен быть в tree!)
        nav_region.bake_navigation_mesh(); // АСИНХРОННЫЙ baking из children

        // 5. Создаём Timer для отложенной проверки результата (baking асинхронный)
        let mut timer = Timer::new_alloc();
        timer.set_wait_time(2.0); // 2 секунды задержка
        timer.set_one_shot(true);

        // ВАЖНО: сначала add_child, потом connect и start!
        self.base_mut().add_child(&timer.clone().upcast::<Node>());

        // Clone nav_region для callback
        let nav_region_for_callback = nav_region.clone();

        // Создаём callable из замыкания
        let check_callback = Callable::from_fn("check_navmesh_bake", move |_args| {
            voidrun_simulation::log_error("⏰ Timer callback triggered!");

            let baked_mesh = nav_region_for_callback.get_navigation_mesh();
            if let Some(mesh) = baked_mesh {
                let vertex_count = mesh.get_vertices().len();
                let polygon_count = mesh.get_polygon_count();
                voidrun_simulation::log_error(&format!(
                    "✅ NavMesh baked (after 2 sec): {} vertices, {} polygons",
                    vertex_count, polygon_count
                ));

                if polygon_count == 0 {
                    voidrun_simulation::log_error(
                        "❌ WARNING: NavMesh has 0 polygons! Check geometry/parameters",
                    );
                } else {
                    voidrun_simulation::log_error(
                        "🎉 NavMesh baking SUCCESS - physical objects detected!",
                    );
                }
            } else {
                voidrun_simulation::log_error("❌ ERROR: Failed to bake NavMesh!");
            }
            Variant::nil()
        });

        // Подключаем timeout signal к callable
        timer.connect("timeout", &check_callback);

        // Запускаем timer
        timer.start();

        voidrun_simulation::log_error(
            "✅ NavigationRegion3D ready, baking in progress (check in 2 sec)...",
        );
    }

    /// Создать lights (directional sun)
    pub(super) fn create_lights(&mut self) {
        let mut light = DirectionalLight3D::new_alloc();
        light.set_rotation_degrees(Vector3::new(-45.0, 0.0, 0.0));
        light.set_param(LightParam::ENERGY, 1.0);

        self.base_mut().add_child(&light.upcast::<Node>());
    }

    /// Создать RTS camera (WASD + mouse orbit + scroll zoom)
    pub(super) fn create_camera(&mut self) {
        let mut camera = Gd::<RTSCamera3D>::from_init_fn(|base| RTSCamera3D::init(base));

        // Начальная позиция камеры
        camera.set_position(Vector3::new(0.0, 5.0, 0.0));
        camera.set_rotation_degrees(Vector3::new(0.0, 0.0, 0.0));

        self.base_mut().add_child(&camera.upcast::<Node>());

        voidrun_simulation::log("RTSCamera3D added - use WASD, RMB drag, mouse wheel");
    }

    /// Создать FPS counter label + Spawn button (top-left corner)
    ///
    /// Returns (fps_label, spawn_button) для хранения references в SimulationBridge
    pub(super) fn create_fps_label(&mut self) -> (Gd<Label>, Gd<Button>) {
        // CanvasLayer для UI overlay (рендерится поверх 3D сцены)
        let mut canvas_layer = CanvasLayer::new_alloc();

    // Label для FPS
    let mut label = Label::new_alloc();
    label.set_text("FPS: --");

    // Позиция: top-left corner
    label.set_position(Vector2::new(10.0, 10.0));

    // Стиль: белый текст, крупный шрифт
    label.add_theme_color_override("font_color", Color::from_rgb(1.0, 1.0, 1.0));
    label.add_theme_font_size_override("font_size", 24);

    // Добавляем label в canvas layer
    canvas_layer.add_child(&label.clone().upcast::<Node>());

    // Button для спавна NPC (под FPS label)
    let mut button = Button::new_alloc();
    button.set_text("Spawn NPCs");
    button.set_position(Vector2::new(10.0, 50.0)); // Под FPS label
    button.set_size(Vector2::new(150.0, 40.0));

    // NOTE: Signal подключается в SimulationBridge::ready() (нужен callable)
    // button.connect("pressed", &callable);

    // Добавляем button в canvas layer
    canvas_layer.add_child(&button.clone().upcast::<Node>());

    // Button для спавна Player (под Spawn NPCs button)
    let mut player_button = Button::new_alloc();
    player_button.set_text("Spawn Player");
    player_button.set_position(Vector2::new(10.0, 100.0)); // Под Spawn NPCs button
    player_button.set_size(Vector2::new(150.0, 40.0));

    // NOTE: Signal подключается в SimulationBridge::ready() (нужен callable)
    // player_button.connect("pressed", &player_callable);

    // Добавляем player button в canvas layer
    canvas_layer.add_child(&player_button.upcast::<Node>());

        // Добавляем canvas layer в сцену
        self.base_mut().add_child(&canvas_layer.upcast::<Node>());

        voidrun_simulation::log("FPS counter + Spawn buttons UI created (top-left corner)");

        (label, button)
    }
}
