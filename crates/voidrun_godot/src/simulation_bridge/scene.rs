//! Scene creation (navigation, lights, camera, UI)
//!
//! Extension методы для SimulationBridge (создание 3D сцены программно).

use super::SimulationBridge;
use crate::camera::rts_camera::RTSCamera3D;
use godot::classes::{
    light_3d::Param as LightParam, CanvasLayer, DirectionalLight3D, Node, Timer,
};
use godot::prelude::*;
use godot::builtin::GString;

impl SimulationBridge {
    /// Создать NavigationRegion3D + NavMesh (baking из SceneTree children)
    ///
    /// TEST: Проверяем что NavMesh запекается из StaticBody3D/CSGBox3D children,
    /// а не из процедурной геометрии (для будущего chunk building).
    pub(super) fn create_navigation_region(&mut self) {
        use crate::chunk_navmesh::{
            create_test_navigation_region_with_obstacles, NavMeshBakingParams,
        };

        // 1. Создаём параметры NavMesh baking
        let mut params = NavMeshBakingParams::default();
        params.baking_aabb = godot::builtin::Aabb {
            position: Vector3::new(-200.0, -1.0, -200.0),
            size: Vector3::new(400.0, 10.0, 400.0), // Высота 10м (для боксов)
        };

        // 2. Создаём NavigationRegion3D с тестовыми obstacles через утилиту
        let mut nav_region = create_test_navigation_region_with_obstacles(&params);

        // 3. Добавляем NavigationRegion3D в сцену ПЕРЕД baking
        self.base_mut()
            .add_child(&nav_region.clone().upcast::<Node>());

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

    /// Создать DebugOverlay (FPS counter, spawn buttons, F3 toggle)
    ///
    /// DebugOverlay — отдельный Control node с всем debug UI.
    /// Создаётся через CanvasLayer для рендеринга поверх 3D сцены.
    pub(super) fn create_debug_overlay(&mut self) {
        // CanvasLayer для UI overlay (рендерится поверх 3D сцены)
        let mut canvas_layer = CanvasLayer::new_alloc();

        // Получаем путь к SimulationBridge
        let bridge_path = self.base().get_path();

        // Создаём DebugOverlay node с передачей пути через bind_mut
        use godot::classes::IControl;
        let mut debug_overlay =
            Gd::<crate::debug_overlay::DebugOverlay>::from_init_fn(|base| {
                <crate::debug_overlay::DebugOverlay as IControl>::init(base)
            });

        // Устанавливаем путь к SimulationBridge ПЕРЕД добавлением в дерево
        let path_string = bridge_path.to_string();
        debug_overlay.bind_mut().simulation_bridge_path = path_string.as_str().into();

        // Устанавливаем anchor preset (full rect — занимает весь экран)
        debug_overlay.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);

        // Добавляем DebugOverlay в canvas layer
        canvas_layer.add_child(&debug_overlay.upcast::<Node>());

        // Добавляем canvas layer в сцену
        self.base_mut().add_child(&canvas_layer.upcast::<Node>());

        voidrun_simulation::log("DebugOverlay created (F3 to toggle)");
    }
}
