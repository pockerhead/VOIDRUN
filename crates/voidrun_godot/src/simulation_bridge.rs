use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, CapsuleMesh, PlaneMesh, SphereMesh,
    StandardMaterial3D, DirectionalLight3D, Label3D,
    Node, Mesh, Material, CpuParticles3D,
    light_3d::Param as LightParam,
    base_material_3d::{BillboardMode, ShadingMode as BaseMaterial3DShading, Flags as BaseMaterial3DFlags},
    cpu_particles_3d::{EmissionShape, Parameter as CpuParam},
};
use voidrun_simulation::*;
use crate::camera::rts_camera::RTSCamera3D;
use voidrun_simulation::ai::AIState;
use voidrun_simulation::combat::{Weapon, WeaponState};

/// Мост между Godot и Rust ECS симуляцией (100% Rust, no GDScript)
///
/// Архитектура:
/// - Создаёт всю 3D сцену программно в ready()
/// - Каждый frame: ECS update → sync transforms → update health bars
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SimulationBridge {
    base: Base<Node3D>,

    /// Bevy ECS App (симуляция)
    simulation: Option<bevy::app::App>,

    /// Визуальные представления NPC [NPC1, NPC2]
    npc_visuals: Vec<Gd<MeshInstance3D>>,

    /// Health bar labels над NPC
    health_labels: Vec<Gd<Label3D>>,

    /// Stamina bar labels под health bar
    stamina_labels: Vec<Gd<Label3D>>,

    /// AI state labels над health bar
    ai_state_labels: Vec<Gd<Label3D>>,

    /// Weapon meshes для каждого NPC
    weapon_meshes: Vec<Gd<MeshInstance3D>>,

    /// Entity indices для синхронизации
    entity_indices: Vec<u32>,
}

#[godot_api]
impl INode3D for SimulationBridge {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            simulation: None,
            npc_visuals: Vec::new(),
            health_labels: Vec::new(),
            stamina_labels: Vec::new(),
            ai_state_labels: Vec::new(),
            weapon_meshes: Vec::new(),
            entity_indices: Vec::new(),
        }
    }

    fn ready(&mut self) {
        godot_print!("SimulationBridge ready - building 3D scene in Rust");

        // 1. Создаём ground plane
        self.create_ground();

        // 2. Создаём lights
        self.create_lights();

        // 3. Создаём camera
        self.create_camera();

        // 4. Инициализируем ECS симуляцию
        let mut app = create_headless_app(42);
        voidrun_simulation::set_logger(Box::new(GodotLogger));
        app.add_plugins(SimulationPlugin);

        // 5. Спавним 2 NPC в симуляции (с разными характеристиками для асимметрии)
        let world = app.world_mut();
        let npc1 = spawn_test_npc(world, (-3.0, 0.5, 0.0), 1, 100, 25); // Faction 1: 100 HP, 25 damage
        let npc2 = spawn_test_npc(world, (3.0, 0.5, 0.0), 2, 80, 30);   // Faction 2: 80 HP, 30 damage (больше урона, меньше HP)

        self.entity_indices.push(npc1.index());
        self.entity_indices.push(npc2.index());

        // 6. Создаём визуалы для NPC
        self.create_npc_visual(0, Color::from_rgb(0.8, 0.2, 0.2)); // Red
        self.create_npc_visual(1, Color::from_rgb(0.2, 0.2, 0.8)); // Blue

        self.simulation = Some(app);

        godot_print!("Scene ready: 2 NPCs spawned with full Rust visuals");
    }

    fn process(&mut self, _delta: f64) {
        // Обновляем симуляцию
        if let Some(app) = &mut self.simulation {
            app.update();

            // Debug: показываем AI states (раз в секунду)
            static mut DEBUG_TIMER: f32 = 0.0;
            unsafe {
                DEBUG_TIMER += _delta as f32;
                if DEBUG_TIMER >= 1.0 {
                    DEBUG_TIMER = 0.0;

                    let world = app.world_mut();
                    let mut query = world.query::<(bevy::prelude::Entity, &AIState, &Actor, &Health, &Stamina)>();

                    for (entity, state, actor, health, stamina) in query.iter(world) {
                        godot_print!("DEBUG: Entity {:?} (faction {}) HP:{}/{} Stamina:{:.0}/{:.0} state = {:?}",
                            entity, actor.faction_id, health.current, health.max, stamina.current, stamina.max, state);
                    }
                }
            }
        }

        // Обрабатываем hit effects (DamageDealt события)
        self.process_hit_effects();

        // Синхронизируем визуалы
        self.sync_visuals();
    }
}

#[godot_api]
impl SimulationBridge {
    /// Создать ground plane (20x20m зелёный)
    fn create_ground(&mut self) {
        let mut mesh_instance = MeshInstance3D::new_alloc();

        // Plane mesh
        let mut plane = PlaneMesh::new_gd();
        plane.set_size(Vector2::new(20.0, 20.0));
        mesh_instance.set_mesh(&plane.upcast::<Mesh>());

        // Зелёный материал
        let mut material = StandardMaterial3D::new_gd();
        material.set_albedo(Color::from_rgb(0.3, 0.5, 0.3));
        mesh_instance.set_surface_override_material(0, &material.upcast::<Material>());

        // Добавляем в сцену
        self.base_mut().add_child(&mesh_instance.upcast::<Node>());
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

        godot_print!("RTSCamera3D added - use WASD, RMB drag, mouse wheel");
    }

    /// Создать визуал NPC (capsule mesh + health/stamina/AI labels + weapon)
    fn create_npc_visual(&mut self, _index: usize, color: Color) {
        // Capsule mesh (тело актора)
        let mut mesh_instance = MeshInstance3D::new_alloc();

        let mut capsule = CapsuleMesh::new_gd();
        capsule.set_radius(0.4);
        capsule.set_height(1.8);
        mesh_instance.set_mesh(&capsule.upcast::<Mesh>());

        // Материал с цветом фракции
        let mut material = StandardMaterial3D::new_gd();
        material.set_albedo(color);
        mesh_instance.set_surface_override_material(0, &material.upcast::<Material>());

        // AI state label (над головой, выше health)
        let mut ai_label = Label3D::new_alloc();
        ai_label.set_text("[Idle]");
        ai_label.set_pixel_size(0.004);
        ai_label.set_billboard_mode(BillboardMode::ENABLED);
        ai_label.set_position(Vector3::new(0.0, 1.4, 0.0));
        ai_label.set_modulate(Color::from_rgb(0.8, 0.8, 0.2)); // Желтый
        mesh_instance.add_child(&ai_label.clone().upcast::<Node>());

        // Health label над головой
        let mut health_label = Label3D::new_alloc();
        health_label.set_text("HP: 100/100");
        health_label.set_pixel_size(0.005);
        health_label.set_billboard_mode(BillboardMode::ENABLED);
        health_label.set_position(Vector3::new(0.0, 1.2, 0.0));
        mesh_instance.add_child(&health_label.clone().upcast::<Node>());

        // Stamina label под health
        let mut stamina_label = Label3D::new_alloc();
        stamina_label.set_text("Stamina: 100/100");
        stamina_label.set_pixel_size(0.004);
        stamina_label.set_billboard_mode(BillboardMode::ENABLED);
        stamina_label.set_position(Vector3::new(0.0, 1.0, 0.0));
        stamina_label.set_modulate(Color::from_rgb(0.2, 0.8, 0.2)); // Зелёный
        mesh_instance.add_child(&stamina_label.clone().upcast::<Node>());

        // Weapon mesh (меч-капсула, child of actor)
        let mut weapon_mesh = MeshInstance3D::new_alloc();
        let mut weapon_capsule = CapsuleMesh::new_gd();
        weapon_capsule.set_radius(0.08); // Тонкий меч
        weapon_capsule.set_height(1.5); // Длина 1.5m (чтобы достать до врага на 2м)
        weapon_mesh.set_mesh(&weapon_capsule.upcast::<Mesh>());

        // Weapon материал (серебристый металл)
        let mut weapon_material = StandardMaterial3D::new_gd();
        weapon_material.set_albedo(Color::from_rgb(0.7, 0.7, 0.8));
        weapon_material.set_metallic(0.9);
        weapon_material.set_roughness(0.2);
        weapon_mesh.set_surface_override_material(0, &weapon_material.upcast::<Material>());

        // Weapon position: впереди актора (на уровне руки), сдвинут вперёд на половину длины
        weapon_mesh.set_position(Vector3::new(0.3, 0.3, 1.0)); // Вправо 0.3, вверх 0.3 (рука), вперёд 1.0

        // Weapon rotation: повернут по диагонали (45° вправо, 30° вниз)
        weapon_mesh.set_rotation_degrees(Vector3::new(-30.0, 0.0, 45.0));

        mesh_instance.add_child(&weapon_mesh.clone().upcast::<Node>());

        // Добавляем в сцену
        self.base_mut().add_child(&mesh_instance.clone().upcast::<Node>());

        self.npc_visuals.push(mesh_instance);
        self.health_labels.push(health_label);
        self.stamina_labels.push(stamina_label);
        self.ai_state_labels.push(ai_label);
        self.weapon_meshes.push(weapon_mesh);
    }

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
                godot_print!("DEBUG: Found {} damage events this frame", events.len());
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
            godot_print!("DEBUG: Spawning hit particles at {:?}", pos);
            self.spawn_hit_particles(pos);
        }
    }

    /// Спавнит красные particles в точке удара
    fn spawn_hit_particles(&mut self, position: Vector3) {
        godot_print!("DEBUG: Creating particles at position {:?}", position);

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

        godot_print!("DEBUG: Particles spawned and added to scene");

        // Автоудаление через 1 секунду (после окончания эффекта)
        // TODO: добавить timer для автоочистки
    }

    fn sync_visuals(&mut self) {
        if let Some(app) = &mut self.simulation {
            let world = app.world();

            for (i, &entity_index) in self.entity_indices.iter().enumerate() {
                let entity = bevy::prelude::Entity::from_raw(entity_index);

                // Синхронизируем transform актора
                if let Some(transform) = world.get::<bevy::prelude::Transform>(entity) {
                    let pos = transform.translation;
                    self.npc_visuals[i].set_position(Vector3::new(pos.x, pos.y, pos.z));
                }

                // Обновляем health label
                if let Some(health) = world.get::<Health>(entity) {
                    let text = format!("HP: {}/{}", health.current, health.max);
                    self.health_labels[i].set_text(&text);
                }

                // Обновляем stamina label
                if let Some(stamina) = world.get::<Stamina>(entity) {
                    let text = format!("Stamina: {:.0}/{:.0}", stamina.current, stamina.max);
                    self.stamina_labels[i].set_text(&text);
                }

                // Обновляем AI state label
                if let Some(ai_state) = world.get::<AIState>(entity) {
                    let text = format!("[{:?}]", ai_state);
                    self.ai_state_labels[i].set_text(&text);
                }

                // Синхронизируем weapon rotation (swing animation)
                if let Some(children) = world.get::<bevy::prelude::Children>(entity) {
                    for &child in children.iter() {
                        if let Some(weapon) = world.get::<Weapon>(child) {
                            // Синхронизируем rotation weapon mesh с Rust weapon transform
                            if let Some(weapon_transform) = world.get::<bevy::prelude::Transform>(child) {
                                let rot = weapon_transform.rotation;
                                let (x, y, z) = rot.to_euler(bevy::math::EulerRot::XYZ);
                                self.weapon_meshes[i].set_rotation(Vector3::new(x, y, z));

                                // DEBUG: показываем weapon state
                                static mut LAST_STATE: Option<WeaponState> = None;
                                unsafe {
                                    if LAST_STATE != Some(weapon.state) {
                                        godot_print!("Weapon {} state: {:?}", i, weapon.state);
                                        LAST_STATE = Some(weapon.state);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Спавн тестового NPC в ECS world
fn spawn_test_npc(
    world: &mut bevy::prelude::World,
    position: (f32, f32, f32),
    faction_id: u64,
    max_hp: u32,
    damage: u32,
) -> bevy::prelude::Entity {
    use bevy::prelude::{Transform as BevyTransform, Vec3};

    let mut commands = world.commands();

    commands.spawn((
        Actor { faction_id },
        BevyTransform::from_translation(Vec3::new(position.0, position.1, position.2)),
        PhysicsBody::default(),
        KinematicController {
            move_speed: 5.0,
            gravity: -9.81,
            grounded: false, // ground_detection система установит правильное значение
        },
        MovementInput {
            direction: Vec3::ZERO,
            jump: false,
        },
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
        AIState::Idle,
        AIConfig {
            detection_range: 10.0,
            retreat_stamina_threshold: 0.2,  // Retreat при stamina < 20%
            retreat_health_threshold: 0.0,   // Retreat при HP < 10% (было 20%)
            retreat_duration: 1.5,            // Быстрее возвращаются в бой
        },
    )).id()
}

struct GodotLogger;

impl LogPrinter for GodotLogger {
    fn log(&self, message: &str) {
        godot_print!("{}", message);
    }
}