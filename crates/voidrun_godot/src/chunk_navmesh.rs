//! Chunk-based NavMesh Building (ADR-006: Procgen World)
//!
//! Утилиты для runtime baking NavigationMesh из процедурной геометрии.
//! Используется для chunk streaming world — каждый chunk запекает свой NavMesh.
//!
//! ## Архитектура:
//! - `NavigationMeshSourceGeometryData3D`: процедурная геометрия (vertices/faces)
//! - `NavigationServer3D::bake_from_source_geometry_data()`: runtime baking
//! - Возвращает готовый `NavigationMesh` для установки в `NavigationRegion3D`
//!
//! ## Почему НЕ bake_navigation_mesh():
//! - Требует StaticBody3D/CSG nodes в SceneTree (не работает для динамической геометрии)
//! - Для процгена нужен прямой контроль над геометрией
//! - Chunk streaming требует runtime generation БЕЗ заранее созданных nodes

use godot::prelude::*;
use godot::classes::{
    NavigationMesh, NavigationServer3D, NavigationMeshSourceGeometryData3D,
};

/// Параметры NavMesh baking (настройки алгоритма)
///
/// Используется для создания NavigationMesh с нужными параметрами.
/// Влияет на качество pathfinding и производительность.
#[derive(Debug, Clone)]
pub struct NavMeshBakingParams {
    /// Размер ячейки сетки (меньше = точнее, но дороже), рекомендуется 0.25м
    pub cell_size: f32,
    /// Высота ячейки (меньше = точнее по вертикали), рекомендуется 0.25м
    pub cell_height: f32,
    /// Высота агента (для проверки проходимости), стандартно 1.8м
    pub agent_height: f32,
    /// Радиус агента (для obstacle avoidance), стандартно 0.5м
    pub agent_radius: f32,
    /// Максимальная высота подъёма (лестницы, ступени), стандартно 0.5м
    pub agent_max_climb: f32,
    /// AABB для baking — ограничивает область генерации NavMesh
    pub baking_aabb: godot::builtin::Aabb,
}

impl Default for NavMeshBakingParams {
    fn default() -> Self {
        Self {
            cell_size: 0.25,
            cell_height: 0.25,
            agent_height: 1.8,
            agent_radius: 0.5,
            agent_max_climb: 0.5,
            // По умолчанию 400x400м плоскость (для тестов)
            baking_aabb: godot::builtin::Aabb {
                position: Vector3::new(-200.0, -1.0, -200.0),
                size: Vector3::new(400.0, 2.0, 400.0),
            },
        }
    }
}

/// Запечь NavigationMesh из процедурной геометрии (runtime baking)
///
/// ## Параметры:
/// - `vertices`: треугольники геометрии (каждые 3 вершины = 1 треугольник, clockwise winding)
/// - `params`: параметры baking алгоритма (cell size, agent размеры, AABB)
///
/// ## Возвращает:
/// - `Gd<NavigationMesh>`: готовый NavMesh для установки в NavigationRegion3D
///
/// ## Пример использования:
/// ```rust
/// let vertices = generate_flat_plane_geometry(400.0, 400.0);
/// let params = NavMeshBakingParams::default();
/// let nav_mesh = bake_navmesh_from_geometry(&vertices, &params);
///
/// let mut nav_region = NavigationRegion3D::new_alloc();
/// nav_region.set_navigation_mesh(&nav_mesh);
/// ```
///
/// ## Для chunk streaming:
/// ```rust
/// // Генерация chunk навмеша (32x32м)
/// let chunk_vertices = generate_chunk_geometry(chunk_coord, world_seed);
/// let mut params = NavMeshBakingParams::default();
/// params.baking_aabb = Aabb {
///     position: Vector3::new(chunk_x * 32.0, -1.0, chunk_z * 32.0),
///     size: Vector3::new(32.0, 2.0, 32.0),
/// };
/// let chunk_navmesh = bake_navmesh_from_geometry(&chunk_vertices, &params);
/// ```
pub fn bake_navmesh_from_geometry(
    vertices: &godot::builtin::PackedVector3Array,
    params: &NavMeshBakingParams,
) -> Gd<NavigationMesh> {
    // 1. Создать NavigationMesh с параметрами
    let mut nav_mesh = NavigationMesh::new_gd();
    nav_mesh.set_cell_size(params.cell_size);
    nav_mesh.set_cell_height(params.cell_height);
    nav_mesh.set_agent_height(params.agent_height);
    nav_mesh.set_agent_radius(params.agent_radius);
    nav_mesh.set_agent_max_climb(params.agent_max_climb);

    // КРИТИЧНО: AABB для baking — ограничиваем область
    nav_mesh.set_filter_baking_aabb(params.baking_aabb);

    // 2. Создать source geometry data
    let mut source_geometry = NavigationMeshSourceGeometryData3D::new_gd();
    source_geometry.add_faces(vertices, Transform3D::IDENTITY);

    voidrun_simulation::log(&format!(
        "📐 NavMesh baking: {} vertices → NavigationServer3D",
        vertices.len()
    ));

    // 3. Bake NavMesh из процедурной геометрии (синхронно)
    let mut nav_server = NavigationServer3D::singleton();
    nav_server.bake_from_source_geometry_data(&nav_mesh, &source_geometry);

    // Debug: проверяем результат
    let vertex_count = nav_mesh.get_vertices().len();
    let polygon_count = nav_mesh.get_polygon_count();
    voidrun_simulation::log(&format!(
        "✅ NavMesh baked: {} vertices, {} polygons",
        vertex_count, polygon_count
    ));

    if polygon_count == 0 {
        voidrun_simulation::log("❌ WARNING: NavMesh has 0 polygons! Check geometry/parameters");
    }

    nav_mesh
}

/// Генерация простой flat plane геометрии (для тестов)
///
/// Создаёт 2 треугольника (quad) для плоскости размером `width x height` метров.
///
/// ## Параметры:
/// - `width`: ширина плоскости по оси X (метры)
/// - `height`: высота плоскости по оси Z (метры)
///
/// ## Возвращает:
/// - `PackedVector3Array`: 6 вершин (2 треугольника, clockwise winding)
pub fn generate_flat_plane_geometry(width: f32, height: f32) -> godot::builtin::PackedVector3Array {
    let mut vertices = godot::builtin::PackedVector3Array::new();

    let half_w = width / 2.0;
    let half_h = height / 2.0;

    // Triangle 1 (clockwise from top):
    vertices.push(Vector3::new(-half_w, 0.0, -half_h)); // top-left
    vertices.push(Vector3::new(half_w, 0.0, -half_h)); // top-right
    vertices.push(Vector3::new(half_w, 0.0, half_h)); // bottom-right

    // Triangle 2:
    vertices.push(Vector3::new(-half_w, 0.0, -half_h)); // top-left
    vertices.push(Vector3::new(half_w, 0.0, half_h)); // bottom-right
    vertices.push(Vector3::new(-half_w, 0.0, half_h)); // bottom-left

    vertices
}

/// Создать test NavigationRegion3D с физическими объектами (SceneTree-based baking)
///
/// **ВАЖНО:** Это TEST функция для проверки что NavMesh запекается из StaticBody3D/CollisionShape3D.
/// Для production chunk building нужно будет генерировать геометрию из chunk data.
///
/// ## Что создаётся:
/// - NavigationRegion3D (родитель)
/// - Ground: StaticBody3D с BoxShape3D (400x1x400м земля)
/// - Obstacles: 4 StaticBody3D бокса разных размеров (тестовые препятствия)
/// - Visual meshes (зелёная земля, красные боксы)
///
/// ## Параметры:
/// - `params`: параметры NavMesh baking
///
/// ## Возвращает:
/// - `Gd<NavigationRegion3D>`: готовый region с NavMesh settings (baking асинхронный!)
///
/// ## Пример использования:
/// ```rust
/// let params = NavMeshBakingParams::default();
/// let mut nav_region = create_test_navigation_region_with_obstacles(&params);
///
/// // Добавить в SceneTree ПЕРЕД baking
/// parent_node.add_child(&nav_region.clone().upcast::<Node>());
///
/// // Запустить асинхронный baking
/// nav_region.bake_navigation_mesh();
///
/// // Результат доступен через ~2 сек или signal "baking_finished"
/// ```
pub fn create_test_navigation_region_with_obstacles(
    params: &NavMeshBakingParams,
) -> Gd<godot::classes::NavigationRegion3D> {
    use godot::classes::{
        NavigationRegion3D, StaticBody3D, CollisionShape3D, BoxShape3D,
        MeshInstance3D, PlaneMesh, BoxMesh, StandardMaterial3D, Mesh, Material, Node,
    };

    // 1. Создать NavigationMesh с параметрами
    let mut nav_mesh = NavigationMesh::new_gd();
    nav_mesh.set_cell_size(params.cell_size);
    nav_mesh.set_cell_height(params.cell_height);
    nav_mesh.set_agent_height(params.agent_height);
    nav_mesh.set_agent_radius(params.agent_radius);
    nav_mesh.set_agent_max_climb(params.agent_max_climb);
    nav_mesh.set_filter_baking_aabb(params.baking_aabb);

    // 2. Создать NavigationRegion3D
    let mut nav_region = NavigationRegion3D::new_alloc();
    nav_region.set_navigation_mesh(&nav_mesh);
    nav_region.set_name("NavRegion_Test");

    // 3. Создать StaticBody3D как землю (400x400м, collision enabled)
    let mut ground_body = StaticBody3D::new_alloc();
    ground_body.set_name("Ground");

    // CollisionShape3D для земли (плоский box 400x1x400м)
    let mut ground_collision = CollisionShape3D::new_alloc();
    let mut ground_shape = BoxShape3D::new_gd();
    ground_shape.set_size(Vector3::new(60.0, 1.0, 60.0));
    ground_collision.set_shape(&ground_shape.upcast::<godot::classes::Shape3D>());
    ground_collision.set_position(Vector3::new(0.0, -0.5, 0.0)); // Опустить на полметра
    ground_body.add_child(&ground_collision.upcast::<Node>());

    // Visual mesh для земли (зелёная плоскость)
    let mut ground_visual = MeshInstance3D::new_alloc();
    ground_visual.set_name("GroundVisual");
    let mut plane = PlaneMesh::new_gd();
    plane.set_size(Vector2::new(60.0, 60.0));
    ground_visual.set_mesh(&plane.upcast::<Mesh>());

    let mut material = StandardMaterial3D::new_gd();
    material.set_albedo(Color::from_rgb(0.3, 0.5, 0.3));
    ground_visual.set_surface_override_material(0, &material.upcast::<Material>());
    ground_body.add_child(&ground_visual.upcast::<Node>());

    nav_region.add_child(&ground_body.upcast::<Node>());

    // 4. Создать несколько StaticBody3D боксов как obstacles (разные размеры)
    let obstacles = [
        // ("Obstacle1", Vector3::new(10.0, 0.0, 10.0), Vector3::new(2.0, 4.0, 2.0)),
        // ("Obstacle2", Vector3::new(-15.0, 0.0, 5.0), Vector3::new(3.0, 4.5, 3.0)),
        // ("Obstacle3", Vector3::new(20.0, 0.0, -10.0), Vector3::new(1.5, 4.0, 1.5)),
        // ("Obstacle4", Vector3::new(-10.0, 0.0, -15.0), Vector3::new(4.0, 4.0, 2.0)),
        ("Obstacle5", Vector3::new(-0.0, 0.0, -0.0), Vector3::new(4.0, 4.0, 4.0)),

    ];

    for (name, pos, size) in obstacles.iter() {
        let mut obstacle = StaticBody3D::new_alloc();
        obstacle.set_name(*name);
        obstacle.set_position(*pos);

        // Collision layers: Environment (layer 3)
        // Obstacles коллидируют с actors (layer 2) и projectiles (layer 4)
        obstacle.set_collision_layer(crate::collision_layers::COLLISION_LAYER_ENVIRONMENT);
        obstacle.set_collision_mask(
            crate::collision_layers::COLLISION_LAYER_ACTORS
                | crate::collision_layers::COLLISION_LAYER_PROJECTILES,
        );

        // CollisionShape3D
        let mut collision = CollisionShape3D::new_alloc();
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(*size);
        collision.set_shape(&shape.upcast::<godot::classes::Shape3D>());
        obstacle.add_child(&collision.upcast::<Node>());

        // Visual mesh (красный box)
        let mut visual = MeshInstance3D::new_alloc();
        let mut box_mesh = BoxMesh::new_gd();
        box_mesh.set_size(*size);
        visual.set_mesh(&box_mesh.upcast::<Mesh>());

        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgb(0.8, 0.2, 0.2)); // Красные obstacles
        visual.set_surface_override_material(0, &mat.upcast::<Material>());
        obstacle.add_child(&visual.upcast::<Node>());

        nav_region.add_child(&obstacle.upcast::<Node>());
    }

    voidrun_simulation::log("🏗️ Test NavigationRegion3D created with ground + 4 obstacles");

    nav_region
}
