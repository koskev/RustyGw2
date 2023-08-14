//! This example shows various ways to configure texture materials in 3D.

use std::{f32::consts::PI, fs, path::Path, time::Instant};
use walkdir::WalkDir;

use bevy::{
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::{shape::Quad, *},
    render::{
        camera::{
            camera_system, CameraProjection, CameraProjectionPlugin, CameraRenderGraph, Viewport,
        },
        mesh::VertexAttributeValues,
        primitives::Frustum,
        render_resource::{AddressMode, SamplerDescriptor},
        view::{update_frusta, ColorGrading, VisibilitySystems, VisibleEntities},
    },
    transform::TransformSystem,
    window::PresentMode,
};
use bevy_mod_billboard::prelude::*;

mod gw2link;
mod gw2poi;
mod processutils;

use gw2link::GW2Link;
use gw2poi::{PoiContainer, TrailContainer};

use crate::gw2poi::OverlayData;

#[derive(Component)]
struct GlobalState {
    gw2link: GW2Link,
}

#[derive(Component)]
struct Gw2Camera;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct FpsText;

#[derive(Resource)]
struct CurrentLevel(u32);

#[derive(Resource)]
struct MapData {
    data: OverlayData,
}

/// A 3D camera projection in which distant objects appear smaller than close objects.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct PerspectiveProjectionGW2 {
    /// The vertical field of view (FOV) in radians.
    ///
    /// Defaults to a value of π/4 radians or 45 degrees.
    pub fov: f32,

    /// The aspect ratio (width divided by height) of the viewing frustum.
    ///
    /// Bevy's [`camera_system`](crate::camera::camera_system) automatically
    /// updates this value when the aspect ratio of the associated window changes.
    ///
    /// Defaults to a value of `1.0`.
    pub aspect_ratio: f32,

    /// The distance from the camera in world units of the viewing frustum's near plane.
    ///
    /// Objects closer to the camera than this value will not be visible.
    ///
    /// Defaults to a value of `0.1`.
    pub near: f32,

    /// The distance from the camera in world units of the viewing frustum's far plane.
    ///
    /// Objects farther from the camera than this value will not be visible.
    ///
    /// Defaults to a value of `1000.0`.
    pub far: f32,
}

impl CameraProjection for PerspectiveProjectionGW2 {
    fn get_projection_matrix(&self) -> Mat4 {
        let mat = Mat4::perspective_infinite_lh(self.fov, self.aspect_ratio, self.near);
        mat
    }

    fn update(&mut self, width: f32, height: f32) {
        self.aspect_ratio = width / height;
    }

    fn far(&self) -> f32 {
        self.far
    }
}

impl Default for PerspectiveProjectionGW2 {
    fn default() -> Self {
        PerspectiveProjectionGW2 {
            fov: std::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
        }
    }
}

fn main() {
    let pid = processutils::find_wine_process("GW2-64.exe");
    info!("Got pid {:?}", pid);
    processutils::start_gw2_helper(pid.unwrap(), "/tmp/mumble.exe");

    // TODO: instead of own plugin just change the attributes etc. of the existing window by
    // getting the raw handle
    App::new()
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_window)
        //.add_systems(Update, rotate_camera)
        .add_systems(Update, update_gw2)
        .add_systems(Update, (update_text_fps, update_text_debug))
        //.add_systems(Update, draw_lines)
        .add_systems(Update, map_change_event)
        .insert_resource(ClearColor(Color::NONE))
        .insert_resource(CurrentLevel(0))
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::winit::WinitPlugin>()
                // Set the sampler mode to repeat for the trails to work
                // https://github.com/bevyengine/bevy/issues/399
                .set(ImagePlugin {
                    default_sampler: SamplerDescriptor {
                        address_mode_u: AddressMode::Repeat,
                        address_mode_v: AddressMode::Repeat,
                        address_mode_w: AddressMode::Repeat,
                        ..Default::default()
                    },
                }),
        )
        .add_plugins(custom_window_plugin::WinitPlugin)
        .add_plugins(BillboardPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_event::<MapChangeEvent>()
        .add_plugins(CameraProjectionPlugin::<PerspectiveProjectionGW2>::default())
        .add_systems(
            PostUpdate,
            update_frusta::<PerspectiveProjectionGW2>
                .in_set(VisibilitySystems::UpdatePerspectiveFrusta)
                .after(camera_system::<PerspectiveProjectionGW2>)
                .after(TransformSystem::TransformPropagate),
        )
        .run();
}

fn setup_window(mut window: Query<&mut Window>) {
    let mut window = window.single_mut();

    window.present_mode = PresentMode::AutoVsync;
    window.resolution.set(1920.0, 1080.0);
}

fn update_gw2(
    mut global_state_query: Query<&mut GlobalState>,
    mut camera_query: Query<&mut Transform, With<Gw2Camera>>,
    mut ev_map_change: EventWriter<MapChangeEvent>,
    mut current_level_query: ResMut<CurrentLevel>,
) {
    let before = Instant::now();
    while global_state_query.single_mut().gw2link.update_gw2(false) {}
    //global_state_query.single_mut().gw2link.update_gw2(false);
    let after = Instant::now();
    let data = global_state_query.single_mut().gw2link.get_gw2_data();

    let mut cam = camera_query.single_mut();
    let camera_pos = Vec3::from_array(data.get_camera_pos());
    let camera_front = Vec3::from_array(data.get_camera_front());

    cam.translation = camera_pos;
    cam.look_to(-camera_front, Vec3::Y);

    let map_id = data.get_context().map_id;
    if current_level_query.0 != map_id {
        current_level_query.0 = map_id;
        ev_map_change.send(MapChangeEvent(map_id));
    }
}

/// sets up a scene with textured entities
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let link = GW2Link::new().unwrap();
    let state = GlobalState { gw2link: link };
    commands.spawn(state);
    // load a texture and retrieve its aspect ratio

    commands.spawn((
        // Create a TextBundle that has a Text with a single section.
        TextBundle::from_section(
            // Accepts a `String` or any type that converts into a `String`, such as `&str`
            "hello\nbevy!",
            TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 100.0,
                color: Color::WHITE,
            },
        ) // Set the alignment of the Text
        .with_text_alignment(TextAlignment::Center)
        // Set the style of the TextBundle itself.
        .with_style(Style {
            position_type: PositionType::Absolute,
            bottom: Val::Px(5.0),
            right: Val::Px(15.0),
            ..default()
        }),
        DebugText,
    ));

    // Text with multiple sections
    commands.spawn((
        // Create a TextBundle that has a Text with a list of sections.
        TextBundle::from_sections([
            TextSection::new(
                "FPS: ",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 60.0,
                    color: Color::WHITE,
                },
            ),
            TextSection::from_style(TextStyle {
                font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                font_size: 60.0,
                color: Color::GOLD,
            }),
        ]),
        FpsText,
    ));

    // camera
    let projection = PerspectiveProjectionGW2 {
        fov: 1.222,
        far: 1000.0,
        ..Default::default()
    };

    commands.spawn((
        CameraRenderGraph::new(bevy::core_pipeline::core_3d::graph::NAME),
        Camera::default(),
        projection,
        VisibleEntities::default(),
        Frustum::default(),
        Transform::default(),
        GlobalTransform::default(),
        Camera3d::default(),
        Tonemapping::default(),
        DebandDither::Enabled,
        ColorGrading::default(),
        Gw2Camera,
    ));

    let path = Path::new("pois");

    let mut overlay_data: OverlayData = OverlayData {
        ..Default::default()
    };
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() && entry.path().extension().unwrap_or_default() == "xml" {
            info!("Found XML file: {:?}", entry.path());
            let file_path = entry.path().to_string_lossy().to_string();
            let data = OverlayData::from_file(&file_path);
            match data {
                Ok(data) => overlay_data.merge(data),
                Err(e) => error!("Failed to load file {} with error {}", file_path, e),
            }
        }
    }
    overlay_data.fill_poi_parents();
    let map_data = MapData { data: overlay_data };
    commands.insert_resource(map_data);
}

fn update_text_fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                // Update the value of the second section
                text.sections[1].value = format!("{value:.2}");
            }
        }
    }
}

fn update_text_debug(
    camera_query: Query<&Transform, With<Gw2Camera>>,
    mut text_query: Query<&mut Text, With<DebugText>>,
) {
    let mut text = text_query.single_mut();
    let transform = camera_query.single();
    text.sections[0].value = format!(
        "X: {:.1} Y: {:.1} Z: {:.1}\n",
        transform.translation.x, transform.translation.y, transform.translation.z,
    );
}

fn draw_lines(mut gizmos: Gizmos) {
    for i in 0..1 {
        gizmos.line(
            Vec3::new(-1000.0, 0.0, (i * 10) as f32),
            Vec3::new(1000.0, 0.0, (i * 10) as f32),
            Color::RED,
        );
        gizmos.line(
            Vec3::new(-1000.0, 0.0, (i * -10) as f32),
            Vec3::new(1000.0, 0.0, (i * -10) as f32),
            Color::RED,
        );
        gizmos.line(
            Vec3::new((i * 10) as f32, 0.0, -1000.0),
            Vec3::new((i * 10) as f32, 0.0, 1000.0),
            Color::BLUE,
        );
        gizmos.line(
            Vec3::new((i * -10) as f32, 0.0, -1000.0),
            Vec3::new((i * -10) as f32, 0.0, 1000.0),
            Color::BLUE,
        );
    }
    gizmos.line(
        Vec3::new(0.0, -1000.0, 0.0),
        Vec3::new(0.0, 1000.0, 0.0),
        Color::GREEN,
    );
}

#[derive(Component)]
struct BevyPOI {
    poi: PoiContainer,
}
#[derive(Component)]
struct BevyTrail {
    trail: TrailContainer,
}

#[derive(Event)]
struct MapChangeEvent(u32);

fn map_change_event(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
    mut ev_map_change: EventReader<MapChangeEvent>,
    pois: Query<(Entity, With<BevyPOI>)>,
    map_data: Res<MapData>,
) {
    for event in ev_map_change.iter() {
        let current_map: u32 = event.0;
        info!("Changed map to {}", current_map);
        pois.iter()
            .for_each(|(entity, _)| commands.entity(entity).despawn());

        map_data.data.pois.poi_list.iter().for_each(|poi_lock| {
            let poi = poi_lock.read().unwrap();
            if current_map == poi.get_map_id().unwrap_or(0) {
                let icon_path = poi.get_icon_file();
                if icon_path.is_none() {
                    error!("Poi {:?} didn't have a icon path!", poi.get_display_name());
                    return ();
                }
                let texture_handle =
                    asset_server.load(icon_path.unwrap().to_string_lossy().replace(r"\", "/"));

                let entity = BevyPOI {
                    poi: poi_lock.clone(),
                };
                commands.spawn((
                    BillboardTextureBundle {
                        texture: billboard_textures
                            .add(BillboardTexture::Single(texture_handle.clone())),
                        mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(2., 2.)).into())),
                        transform: Transform::from_xyz(
                            poi.pos.xpos,
                            poi.pos.ypos + poi.get_height_offset().unwrap_or(0.0),
                            poi.pos.zpos,
                        ),
                        ..default()
                    },
                    entity,
                ));
            }
        });

        info!("Number of trails: {}", map_data.data.pois.trail_list.len());
        map_data.data.pois.trail_list.iter().for_each(|trail_lock| {
            let trail = trail_lock.read().unwrap();
            if current_map == trail.poi.get_map_id().unwrap_or(0) {
                let texture = trail.texture.clone();
                let texture_handle =
                    asset_server.load(texture.to_string_lossy().replace(r"\", "/"));

                let entity = BevyTrail {
                    trail: trail_lock.clone(),
                };
                let trail_meshes = trail.generate_meshes();

                let pbr_bundles: Vec<PbrBundle> = trail_meshes
                    .into_iter()
                    .map(|mesh| PbrBundle {
                        mesh: meshes.add(mesh),
                        material: materials.add(StandardMaterial {
                            base_color_texture: Some(texture_handle.clone()),
                            unlit: true,
                            cull_mode: None,
                            alpha_mode: AlphaMode::Blend,
                            ..default()
                        }),
                        ..default()
                    })
                    .collect();

                for bundle in pbr_bundles {
                    commands.spawn(bundle);
                }
            }
        });
    }
}
