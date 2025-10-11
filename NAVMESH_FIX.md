# NavMesh проблема: 0 polygons

## Текущая ситуация
- CSGBox3D не работает для NavMesh baking даже с delay
- Нужно попробовать StaticBody3D + CollisionShape3D

## Изменения для simulation_bridge.rs create_navigation_region()

Заменить CSGBox3D на StaticBody3D:

```rust
// Ground: StaticBody3D + CollisionShape3D + BoxShape3D (для NavMesh baking)
let mut ground_body = StaticBody3D::new_alloc();
ground_body.set_position(Vector3::new(0.0, -0.5, 0.0));

// CollisionShape3D с BoxShape3D
let mut collision_shape = CollisionShape3D::new_alloc();
let mut box_shape = BoxShape3D::new_gd();
box_shape.set_size(Vector3::new(400.0, 1.0, 400.0));
collision_shape.set_shape(&box_shape.upcast());
ground_body.add_child(&collision_shape.upcast::<Node>());

// Visual mesh (отдельно от collision)
let mut ground_mesh = MeshInstance3D::new_alloc();
let mut plane = PlaneMesh::new_gd();
plane.set_size(Vector2::new(400.0, 400.0));
ground_mesh.set_mesh(&plane.upcast::<Mesh>());
// материал...
ground_body.add_child(&ground_mesh.upcast::<Node>());

// Добавляем ground_body как child NavigationRegion (для baking)
nav_region.add_child(&ground_body.upcast::<Node>());
```

И поменять ParsedGeometryType:
```rust
nav_mesh.set_parsed_geometry_type(ParsedGeometryType::STATIC_COLLIDERS);
```

Вместо MESH_INSTANCES.
