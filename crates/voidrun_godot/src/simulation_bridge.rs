use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, PlaneMesh, SphereMesh,
    StandardMaterial3D, DirectionalLight3D,
    Node, Mesh, Material, CpuParticles3D,
    NavigationRegion3D,
    light_3d::Param as LightParam,
    base_material_3d::{ShadingMode as BaseMaterial3DShading, Flags as BaseMaterial3DFlags},
    cpu_particles_3d::{EmissionShape, Parameter as CpuParam},
};
use voidrun_simulation::*;
use voidrun_simulation::combat::Weapon;
use crate::camera::rts_camera::RTSCamera3D;
use crate::systems::{VisualRegistry, AttachmentRegistry, SceneRoot, VisionTracking};
use voidrun_simulation::ai::{AIState, SpottedEnemies};
use bevy::ecs::schedule::IntoScheduleConfigs;
use godot::classes::Timer;

/// Мост между Godot и Rust ECS симуляцией (100% Rust, no GDScript)
///
/// Архитектура:
/// - Создаёт всю 3D сцену программно в ready()
/// - Каждый frame: ECS update → sync transforms → update health bars
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
        voidrun_simulation::set_log_level(LogLevel::Error);
        voidrun_simulation::log("SimulationBridge ready - building 3D scene in Rust");

        // 1. Создаём navigation region + ground
        self.create_navigation_region();

        // 2. Создаём lights
        self.create_lights();

        // 3. Создаём camera
        self.create_camera();

        // 4. Инициализируем ECS симуляцию
        let mut app = create_headless_app(42);
        app.add_plugins(SimulationPlugin);

        // 4.1 Регистрируем NonSend resources (main thread only)
        app.insert_non_send_resource(VisualRegistry::default());
        app.insert_non_send_resource(AttachmentRegistry::default());
        app.insert_non_send_resource(VisionTracking::default());
        app.insert_non_send_resource(SceneRoot {
            node: self.base().clone().upcast::<Node3D>(),
        });

        // 4.2 Инициализируем static queues для Godot → ECS events
        crate::projectile::init_projectile_hit_queue();

        // 4.3 Регистрируем visual sync systems (_main_thread = Godot API)
        use crate::systems::{
            spawn_actor_visuals_main_thread,
            sync_health_labels_main_thread,
            sync_stamina_labels_main_thread,
            sync_ai_state_labels_main_thread,
            disable_collision_on_death_main_thread,
            despawn_actor_visuals_main_thread,
            // УДАЛЕНО: sync_transforms_main_thread (ADR-005)
            attach_prefabs_main_thread,
            detach_prefabs_main_thread,
            poll_vision_cones_main_thread,
            weapon_aim_main_thread,
            process_weapon_fire_intents_main_thread,
            weapon_fire_main_thread,
            process_godot_projectile_hits,
            process_movement_commands_main_thread,
            apply_navigation_velocity_main_thread,
            // УДАЛЕНО: sync_strategic_position_from_godot (заменён на event-driven)
        };


        // 3. Sync (Changed<T> → обновление визуалов) + Vision polling + Weapon systems + Movement
        app.add_systems(
            bevy::prelude::Update,
            (
                spawn_actor_visuals_main_thread,
                attach_prefabs_main_thread,
                detach_prefabs_main_thread,
                // УДАЛЕНО: sync_strategic_position_from_godot (заменён на event-driven в apply_navigation_velocity)
                poll_vision_cones_main_thread,    // VisionCone → GodotAIEvent
                process_movement_commands_main_thread, // MovementCommand → NavigationAgent3D
                apply_navigation_velocity_main_thread, // NavigationAgent → CharacterBody + PositionChanged event
                weapon_aim_main_thread,           // Aim RightHand at target
                process_weapon_fire_intents_main_thread, // WeaponFireIntent → tactical validation → WeaponFired
                weapon_fire_main_thread,          // WeaponFired → spawn GodotProjectile
                                                  // Projectile physics → GodotProjectile::_physics_process
                process_godot_projectile_hits,    // Godot queue → ECS ProjectileHit events
                sync_health_labels_main_thread,
                sync_stamina_labels_main_thread,
                sync_ai_state_labels_main_thread,
                disable_collision_on_death_main_thread, // Отключение collision + gray + DespawnAfter
                despawn_actor_visuals_main_thread, // Удаление Godot nodes для despawned entities
            ).chain(),
        );

        // 5. Создаём маркер для отложенного спавна NPC (через 5 секунд)
        app.world_mut().spawn(SpawnNPCsAfter { spawn_time: 5.0 });

        // Регистрируем систему отложенного спавна
        app.add_systems(bevy::prelude::Update, delayed_npc_spawn_system);

        self.simulation = Some(app);

        voidrun_simulation::log("Scene ready: NPCs will spawn after 5 sec (delayed spawn)");
    }

    fn process(&mut self, delta: f64) {
        // Обновляем симуляцию
        if let Some(app) = &mut self.simulation {
            // Передаём delta time в Bevy (для movement system)
            app.world_mut().insert_resource(crate::systems::GodotDeltaTime(delta as f32));
            app.update(); // ECS systems выполнятся, включая attach/detach_prefabs_main_thread
        }

        // Debug: показываем AI states (раз в секунду)
        if let Some(app) = &mut self.simulation {
            static mut DEBUG_TIMER: f32 = 0.0;
            unsafe {
                DEBUG_TIMER += delta as f32;
                if DEBUG_TIMER >= 1.0 {
                    DEBUG_TIMER = 0.0;

                    let world = app.world_mut();
                    let mut query = world.query::<(bevy::prelude::Entity, &AIState, &Actor, &Health, &Stamina)>();

                    for (entity, state, actor, health, stamina) in query.iter(world) {
                        voidrun_simulation::log(&format!("DEBUG: Entity {:?} (faction {}) HP:{}/{} Stamina:{:.0}/{:.0} state = {:?}",
                            entity, actor.faction_id, health.current, health.max, stamina.current, stamina.max, state));
                    }
                }
            }
        }

        // Обрабатываем hit effects (DamageDealt события)
        self.process_hit_effects();

        // Visual sync теперь через ECS systems (_main_thread)
        // sync_health_labels_main_thread, sync_stamina_labels_main_thread, etc.
    }
}

#[godot_api]
impl SimulationBridge {
    /// Создать NavigationRegion3D + NavMesh (baking из SceneTree children)
    ///
    /// TEST: Проверяем что NavMesh запекается из StaticBody3D/CSGBox3D children,
    /// а не из процедурной геометрии (для будущего chunk building).
    fn create_navigation_region(&mut self) {
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
                    voidrun_simulation::log_error("❌ WARNING: NavMesh has 0 polygons! Check geometry/parameters");
                } else {
                    voidrun_simulation::log_error("🎉 NavMesh baking SUCCESS - physical objects detected!");
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

        voidrun_simulation::log_error("✅ NavigationRegion3D ready, baking in progress (check in 2 sec)...");
    }

    /// Создать lights (directional sun)
    fn create_lights(&mut self) {
        let mut light = DirectionalLight3D::new_alloc();
        light.set_rotation_degrees(Vector3::new(-45.0, 0.0, 0.0));
        light.set_param(LightParam::ENERGY, 1.0);

        self.base_mut().add_child(&light.upcast::<Node>());
    }

    /// Создать RTS camera (WASD + mouse orbit + scroll zoom)
    fn create_camera(&mut self) {
        let mut camera = Gd::<RTSCamera3D>::from_init_fn(|base| {
            RTSCamera3D::init(base)
        });

        // Начальная позиция камеры
        camera.set_position(Vector3::new(0.0, 5.0, 0.0));
        camera.set_rotation_degrees(Vector3::new(0.0, 0.0, 0.0));

        self.base_mut().add_child(&camera.upcast::<Node>());

        voidrun_simulation::log("RTSCamera3D added - use WASD, RMB drag, mouse wheel");
    }

    // ❌ REMOVED: create_npc_visual() — теперь используем spawn_actor_visuals_main_thread() ECS систему
    // См. crates/voidrun_godot/src/systems/visual_sync.rs

    /// Синхронизировать визуалы с ECS state
    /// Обрабатывает DamageDealt события и спавнит визуальные эффекты ударов
    fn process_hit_effects(&mut self) {
        use voidrun_simulation::combat::DamageDealt;

        // Сначала собираем позиции для particles (без mutable borrow self)
        let positions: Vec<Vector3> = if let Some(app) = &mut self.simulation {
            let world = app.world();

            // Читаем все DamageDealt события из этого фрейма
            let damage_events = world.resource::<bevy::prelude::Events<DamageDealt>>();

            let events: Vec<DamageDealt> = damage_events
                .iter_current_update_events()
                .cloned()
                .collect();

            if !events.is_empty() {
                voidrun_simulation::log(&format!("DEBUG: Found {} damage events this frame", events.len()));
            }

            // Собираем позиции для particles
            events.iter()
                .filter_map(|event| {
                    world.get::<bevy::prelude::Transform>(event.target)
                        .map(|t| Vector3::new(t.translation.x, t.translation.y + 0.5, t.translation.z))
                })
                .collect()
        } else {
            Vec::new()
        };

        // Теперь спавним particles (можем заимствовать self mutably)
        for pos in positions {
            voidrun_simulation::log(&format!("DEBUG: Spawning hit particles at {:?}", pos));
            self.spawn_hit_particles(pos);
        }
    }

    /// Спавнит красные particles в точке удара
    fn spawn_hit_particles(&mut self, position: Vector3) {
        voidrun_simulation::log(&format!("DEBUG: Creating particles at position {:?}", position));

        let mut particles = CpuParticles3D::new_alloc();

        // Position
        particles.set_position(position);

        // Mesh для частиц (маленькие сферы)
        let mut sphere_mesh = SphereMesh::new_gd();
        sphere_mesh.set_radius(0.08);
        sphere_mesh.set_height(0.16);
        particles.set_mesh(&sphere_mesh.upcast::<Mesh>());

        // Материал с цветом (КРИТИЧНО: vertex_color_use_as_albedo = true!)
        let mut material = StandardMaterial3D::new_gd();
        material.set_flag(BaseMaterial3DFlags::ALBEDO_FROM_VERTEX_COLOR, true);
        material.set_albedo(Color::from_rgb(1.0, 0.2, 0.2));
        material.set_shading_mode(BaseMaterial3DShading::UNSHADED); // Яркий красный без теней
        particles.set_material_override(&material.upcast::<Material>());
        // Настройки emission
        particles.set_emitting(true);
        particles.set_one_shot(true);
        particles.set_explosiveness_ratio(1.0); // Все частицы сразу
        particles.set_amount(30); // 30 частиц
        particles.set_lifetime(0.8); // 0.8 секунды живут

        // Форма emission (sphere)
        particles.set_emission_shape(EmissionShape::SPHERE);
        particles.set_emission_sphere_radius(0.3);

        // Физика частиц
        particles.set_direction(Vector3::new(0.0, 1.0, 0.0));
        particles.set_spread(60.0); // Угол разброса
        particles.set_param_min(CpuParam::INITIAL_LINEAR_VELOCITY, 3.0);
        particles.set_param_max(CpuParam::INITIAL_LINEAR_VELOCITY, 5.0);
        particles.set_gravity(Vector3::new(0.0, -9.8, 0.0));

        // Размер частиц
        particles.set_param_min(CpuParam::SCALE, 0.15);
        particles.set_param_max(CpuParam::SCALE, 0.3);

        // Добавляем в сцену
        self.base_mut().add_child(&particles.upcast::<Node>());

        voidrun_simulation::log("DEBUG: Particles spawned and added to scene");

        // Автоудаление через 1 секунду (после окончания эффекта)
        // TODO: добавить timer для автоочистки
    }

    // УДАЛЕНО: sync_visuals() — заменено на ECS systems:
    // - sync_health_labels_main_thread
    // - sync_stamina_labels_main_thread
    // - sync_ai_state_labels_main_thread
    // - sync_transforms_main_thread
}

/// Компонент-маркер: отложенный спавн NPC
#[derive(bevy::prelude::Component, Debug)]
struct SpawnNPCsAfter {
    spawn_time: f32, // Через сколько секунд спавнить
}

/// Система: отложенный спавн NPC
///
/// Ждёт указанное время и спавнит всех тестовых NPC.
fn delayed_npc_spawn_system(
    mut commands: bevy::prelude::Commands,
    query: bevy::prelude::Query<(bevy::prelude::Entity, &SpawnNPCsAfter)>,
    time: bevy::prelude::Res<bevy::prelude::Time>,
) {
    let elapsed = time.elapsed_secs();

    for (entity, spawn_marker) in query.iter() {
        if elapsed >= spawn_marker.spawn_time {
            voidrun_simulation::log("⏰ Spawning NPCs (delayed spawn triggered)");

            // Спавним всех NPC
            spawn_test_npc(&mut commands, (-25.0, 0.5, -5.0), 1, 100, 1);
            spawn_test_npc(&mut commands, (-16.0, 0.5, -7.0), 1, 100, 1);
            spawn_test_npc(&mut commands, (-18.0, 0.5, -5.0), 1, 100, 1);
            spawn_test_npc(&mut commands, (-5.0, 0.5, -7.0), 1, 100, 1);
            spawn_test_npc(&mut commands, (-7.0, 0.5, -5.0), 1, 100, 1);
            spawn_test_npc(&mut commands, (-4.0, 0.5, -7.0), 1, 100, 1);

            spawn_test_npc(&mut commands, (25.0, 0.5, -5.0), 2, 100, 1);
            spawn_test_npc(&mut commands, (3.0, 0.5, -7.0), 2, 100, 2);
            spawn_test_npc(&mut commands, (5.0, 0.5, -5.0), 2, 100, 1);
            spawn_test_npc(&mut commands, (16.0, 0.5, -7.0), 2, 100, 2);
            spawn_test_npc(&mut commands, (17.0, 0.5, -5.0), 2, 100, 1);
            spawn_test_npc(&mut commands, (18.0, 0.5, -7.0), 2, 100, 2);

            spawn_test_npc(&mut commands, (0.0, 0.5, 7.0), 3, 100, 1);
            spawn_test_npc(&mut commands, (2.0, 0.5, 8.0), 3, 100, 2);
            spawn_test_npc(&mut commands, (0.0, 0.5, 9.0), 3, 100, 1);
            spawn_test_npc(&mut commands, (2.0, 0.5, 8.0), 3, 100, 2);
            spawn_test_npc(&mut commands, (0.0, 0.5, 15.0), 3, 100, 1);
            spawn_test_npc(&mut commands, (2.0, 0.5, 18.0), 3, 100, 2);

            // Удаляем маркер (spawn уже выполнен)
            commands.entity(entity).despawn();

            voidrun_simulation::log("✅ NPCs spawned successfully");
        }
    }
}

/// Спавн тестового NPC в ECS world (ADR-005: StrategicPosition + PrefabPath)
fn spawn_test_npc(
    commands: &mut bevy::prelude::Commands,
    position: (f32, f32, f32), // World position (будет конвертирован в StrategicPosition)
    faction_id: u64,
    max_hp: u32,
    damage: u32,
) -> bevy::prelude::Entity {
    use bevy::prelude::Vec3;

    let world_pos = Vec3::new(position.0, position.1, position.2);
    let strategic_pos = StrategicPosition::from_world_position(world_pos);

    commands.spawn((
        Actor { faction_id },
        strategic_pos, // StrategicPosition (sync_strategic_position_from_godot обновит из Godot)
        PrefabPath::new("res://actors/test_actor.tscn"), // Data-driven prefab path
        Health {
            current: max_hp,
            max: max_hp,
        },
        Stamina {
            current: 100.0,
            max: 100.0,
            regen_rate: 10.0, // 10 stamina/sec
        },
        Attacker {
            attack_cooldown: 1.0,
            cooldown_timer: 0.0,
            base_damage: damage,
            attack_radius: 2.0,
        },
        Weapon::default(), // Weapon system (pistol)
        MovementCommand::Idle, // Godot будет читать и выполнять
        NavigationState::default(), // Трекинг достижения navigation target (для PositionChanged events)
        AIState::Idle,
        AIConfig {
            retreat_stamina_threshold: 0.2,  // Retreat при stamina < 20%
            retreat_health_threshold: 0.0,   // Retreat при HP < 10% (было 20%)
            retreat_duration: 1.5,            // Быстрее возвращаются в бой
            patrol_direction_change_interval: 3.0, // Каждые 3 сек новое направление
        },
        SpottedEnemies::default(), // Godot VisionCone → GodotAIEvent → обновляет список
        Attachment {
            prefab_path: "res://actors/test_pistol.tscn".to_string(),
            attachment_point: "RightHand/WeaponAttachment".to_string(),
            attachment_type: AttachmentType::Weapon,
        },
    )).id()
}

struct GodotLogger;

impl LogPrinter for GodotLogger {
    fn log(&self, level: LogLevel, message: &str) {
        if level >= *voidrun_simulation::LOGGER_LEVEL.lock().unwrap() {
            self._log_message(level, message);
        }
    }
}

impl GodotLogger {
    fn clear_log_file() {
        let log_path = std::path::Path::new("../logs/game.log");
        if let Some(_parent) = log_path.parent() {
            let _ = std::fs::remove_file(log_path);
        }
    }

    fn _log_message(&self, level: LogLevel, message: &str) {
        use std::io::Write;
        if level == LogLevel::Error {
            godot_error!("[{}] {}", level.as_str(), message);
        } else {
            godot_print!("[{}] {}", level.as_str(), message);
        }

        // Пишем в файл logs/game.log (append mode)
        // Godot запускается из godot/ директории, поэтому путь относительно project root
        let log_path = std::path::Path::new("../logs/game.log");

        // Создаём директорию если не существует
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", message);
            }
            Err(e) => {
                // Логируем ошибку только один раз (первый раз)
                static mut ERROR_LOGGED: bool = false;
                unsafe {
                    if !ERROR_LOGGED {
                        godot_error!("❌ Failed to open log file {:?}: {}", log_path, e);
                        ERROR_LOGGED = true;
                    }
                }
            }
        }
    }
}