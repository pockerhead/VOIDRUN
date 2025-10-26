//! Visual effects (hit particles, damage feedback)
//!
//! Extension методы для SimulationBridge (обработка DamageDealt events и visual effects).

use super::SimulationBridge;
use godot::classes::{
    base_material_3d::{Flags as BaseMaterial3DFlags, ShadingMode as BaseMaterial3DShading},
    cpu_particles_3d::{EmissionShape, Parameter as CpuParam},
    CpuParticles3D, Mesh, Node, SphereMesh, StandardMaterial3D,
};
use godot::prelude::*;
use voidrun_simulation::logger;
impl SimulationBridge {
    /// Спавнит красные particles в точке удара
    fn spawn_hit_particles(&mut self, position: Vector3) {
        logger::log(&format!(
            "DEBUG: Creating particles at position {:?}",
            position
        ));

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
        particles.set_material_override(&material.upcast::<godot::classes::Material>());

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

        logger::log("DEBUG: Particles spawned and added to scene");

        // TODO: добавить timer для автоочистки (после 1 секунды)
    }

    /// Обрабатывает DamageDealt события и спавнит визуальные эффекты ударов
    pub(super) fn process_hit_effects(&mut self) {
        use voidrun_simulation::combat::DamageDealt;

        let Some(app) = &mut self.simulation else {
            return;
        };

        // Сначала собираем позиции для particles (без mutable borrow app)
        let positions: Vec<Vector3> = {
            let world = app.world();

            // Читаем все DamageDealt события из этого фрейма
            let damage_events = world.resource::<bevy::prelude::Events<DamageDealt>>();

            let events: Vec<DamageDealt> = damage_events
                .iter_current_update_events()
                .cloned()
                .collect();

            if !events.is_empty() {
                logger::log(&format!(
                    "DEBUG: Found {} damage events this frame",
                    events.len()
                ));
            }

            // Собираем позиции для particles
            events
                .iter()
                .filter_map(|event| {
                    world
                        .get::<bevy::prelude::Transform>(event.target)
                        .map(|t| {
                            Vector3::new(t.translation.x, t.translation.y + 0.5, t.translation.z)
                        })
                })
                .collect()
        };

        // Теперь спавним particles (можем заимствовать self mutably)
        for pos in positions {
            logger::log(&format!("DEBUG: Spawning hit particles at {:?}", pos));
            self.spawn_hit_particles(pos);
        }
    }
}
