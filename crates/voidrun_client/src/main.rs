use bevy::prelude::*;
use voidrun_simulation::{SimulationPlugin, Actor, Health, Stamina};

mod rendering;
mod camera;

use rendering::RenderingSyncPlugin;
use camera::CameraPlugin;

fn main() {
    App::new()
        // Bevy defaults (rendering, input, time, etc.)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "VOIDRUN - Vertical Slice".to_string(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }))
        // Simulation (headless ECS logic)
        .add_plugins(SimulationPlugin)
        // Rendering sync (simulation → visuals)
        .add_plugins(RenderingSyncPlugin)
        // Camera controls
        .add_plugins(CameraPlugin)
        // Setup scene
        .add_systems(Startup, setup_scene)
        .run();
}

/// Spawn ground plane, lights, and 2 NPC fighters
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground plane (10x10m)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Directional light (sun)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
    ));

    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
        affects_lightmapped_meshes: false,
    });

    // Camera (orbit around origin)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(10.0, 8.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        camera::OrbitCamera {
            distance: 15.0,
            ..default()
        },
    ));

    // Spawn 2 NPC fighters (simulation entities)
    // These will be synced to visual entities by RenderingSyncPlugin
    spawn_npc_fighter(&mut commands, Vec3::new(-3.0, 0.5, 0.0), 1);
    spawn_npc_fighter(&mut commands, Vec3::new(3.0, 0.5, 0.0), 2);
}

/// Spawn NPC fighter (simulation entity + visual marker)
fn spawn_npc_fighter(commands: &mut Commands, position: Vec3, faction_id: u64) {
    use voidrun_simulation::{
        PhysicsBody, KinematicController, MovementInput,
        Attacker, AIState, AIConfig,
    };

    commands.spawn((
        Actor { faction_id },
        // Health/Stamina added via Required Components
        Transform::from_translation(position),
        PhysicsBody::default(),
        KinematicController {
            move_speed: 5.0,
            gravity: -9.81,
            grounded: true,
        },
        MovementInput {
            direction: Vec3::ZERO,
            jump: false,
        },
        Attacker {
            attack_cooldown: 1.0,
            cooldown_timer: 0.0,
            base_damage: 20,
            attack_radius: 2.0,
        },
        AIState::Idle,
        AIConfig {
            detection_range: 10.0,
            retreat_stamina_threshold: 0.3,
            retreat_health_threshold: 0.2,
            retreat_duration: 2.0,
        },
        rendering::NeedsVisual, // Marker: spawn visual representation
    ));
}
