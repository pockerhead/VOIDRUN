use bevy::prelude::*;
use voidrun_simulation::{Health, Actor};

pub struct RenderingSyncPlugin;

impl Plugin for RenderingSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            spawn_visuals_for_new_entities,
            sync_transforms,
            update_health_bars,
            despawn_dead_entities,
        ).chain());
    }
}

/// Marker: simulation entity needs visual representation
#[derive(Component)]
pub struct NeedsVisual;

/// Link: visual entity → simulation entity
#[derive(Component)]
pub struct VisualOf(pub Entity);

/// Link: simulation entity → visual entity
#[derive(Component)]
pub struct HasVisual(pub Entity);

/// Health bar visual (UI element positioned above NPC)
#[derive(Component)]
pub struct HealthBar {
    pub owner: Entity,
}

/// Spawn visual representation (capsule mesh) for new simulation entities
fn spawn_visuals_for_new_entities(
    mut commands: Commands,
    query: Query<(Entity, &Actor, &Transform, &Health), With<NeedsVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (sim_entity, actor, sim_transform, _health) in query.iter() {
        // Spawn visual entity (capsule)
        let visual_entity = commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.4, 1.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: faction_color(actor.faction_id),
                ..default()
            })),
            *sim_transform,
            VisualOf(sim_entity),
        )).id();

        // Spawn health bar (child of visual entity)
        let health_bar = commands.spawn((
            HealthBar { owner: sim_entity },
            Transform::from_xyz(0.0, 1.2, 0.0), // Above NPC head
            Visibility::default(),
        )).id();

        // Link simulation ↔ visual
        commands.entity(sim_entity)
            .remove::<NeedsVisual>()
            .insert(HasVisual(visual_entity));

        commands.entity(visual_entity).add_child(health_bar);
    }
}

/// Sync simulation transforms → visual transforms
fn sync_transforms(
    sim_query: Query<(&Transform, &HasVisual), Changed<Transform>>,
    mut visual_query: Query<&mut Transform, (With<VisualOf>, Without<HasVisual>)>,
) {
    for (sim_transform, has_visual) in sim_query.iter() {
        if let Ok(mut visual_transform) = visual_query.get_mut(has_visual.0) {
            *visual_transform = *sim_transform;
        }
    }
}

/// Update health bar UI (simple colored bar above NPC)
fn update_health_bars(
    health_query: Query<(&Health, &HasVisual), Changed<Health>>,
    mut bar_query: Query<(&HealthBar, &mut Transform, &mut Visibility)>,
    mut gizmos: Gizmos,
) {
    for (health, has_visual) in health_query.iter() {
        // Find health bar child
        for (bar, transform, mut visibility) in bar_query.iter_mut() {
            if bar.owner != has_visual.0 {
                continue;
            }

            let health_percent = health.current as f32 / health.max as f32;

            if health_percent <= 0.0 {
                *visibility = Visibility::Hidden;
            } else {
                *visibility = Visibility::Visible;

                // Draw health bar using gizmos (simple debug visualization)
                let bar_width = 1.0;
                let bar_height = 0.1;
                let world_pos = transform.translation;

                // Background (red) - using Isometry3d::from_translation + rotation
                let bg_iso = bevy::math::Isometry3d::new(
                    world_pos,
                    Quat::IDENTITY,
                );
                gizmos.rect(
                    bg_iso,
                    Vec2::new(bar_width, bar_height),
                    Color::srgb(0.8, 0.2, 0.2),
                );

                // Foreground (green, scaled by health)
                let fg_pos = world_pos - Vec3::X * (bar_width * (1.0 - health_percent) * 0.5);
                let fg_iso = bevy::math::Isometry3d::new(
                    fg_pos,
                    Quat::IDENTITY,
                );
                gizmos.rect(
                    fg_iso,
                    Vec2::new(bar_width * health_percent, bar_height),
                    Color::srgb(0.2, 0.8, 0.2),
                );
            }
        }
    }
}

/// Despawn visual entities when simulation entity dies
fn despawn_dead_entities(
    mut commands: Commands,
    dead_query: Query<&HasVisual, (With<Health>, Without<Actor>)>, // Actor removed on death
) {
    for has_visual in dead_query.iter() {
        commands.entity(has_visual.0).despawn(); // Bevy 0.16: despawn() is recursive by default
    }
}

/// Get faction color (simple palette)
fn faction_color(faction_id: u64) -> Color {
    match faction_id % 5 {
        0 => Color::srgb(0.8, 0.2, 0.2), // Red
        1 => Color::srgb(0.2, 0.2, 0.8), // Blue
        2 => Color::srgb(0.2, 0.8, 0.2), // Green
        3 => Color::srgb(0.8, 0.8, 0.2), // Yellow
        _ => Color::srgb(0.8, 0.2, 0.8), // Magenta
    }
}
