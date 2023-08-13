//! This example shows various ways to configure texture materials in 3D.

use std::{f32::consts::PI, fs, time::Instant};

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::{shape::Quad, *},
    window::PresentMode,
};
use bevy_mod_billboard::prelude::*;

mod gw2link;
mod gw2poi;
mod processutils;

use gw2link::GW2Link;
use gw2poi::{PoiTrait, POI};

use crate::gw2poi::OverlayData;

// Trait for extending std::path::PathBuf
use path_slash::PathBufExt as _;

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

#[derive(Component)]
struct DebugObject;

fn main() {
    let pid = processutils::find_wine_process("GW2-64.exe");
    println!("Got pid {:?}", pid);
    processutils::start_gw2_helper(pid.unwrap(), "/tmp/mumble.exe");

    // TODO: instead of own plugin just change the attributes etc. of the existing window by
    // getting the raw handle
    App::new()
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_window)
        .add_systems(Startup, load_poi)
        //.add_systems(Update, rotate_camera)
        .add_systems(Update, update_gw2)
        .add_systems(Update, (update_text_fps, update_text_debug))
        .add_systems(Update, draw_lines)
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins(DefaultPlugins.build().disable::<bevy::winit::WinitPlugin>())
        .add_plugins(custom_window_plugin::WinitPlugin)
        .add_plugins(BillboardPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
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
    _time: Res<Time>,
) {
    let before = Instant::now();
    while global_state_query.single_mut().gw2link.update_gw2(false) {}
    //global_state_query.single_mut().gw2link.update_gw2(false);
    let after = Instant::now();
    let data = global_state_query.single_mut().gw2link.get_gw2_data();

    let mut cam = camera_query.single_mut();
    let mut camera_pos = Vec3::from_array(data.get_camera_pos());
    let mut camera_front = Vec3::from_array(data.get_camera_front());

    camera_pos.z *= -1.0;
    camera_front.z *= -1.0;
    let transform = Transform::from_matrix(bevy::math::f32::Mat4::look_at_lh(
        camera_pos,
        camera_pos + camera_front,
        Vec3::Y,
    ));

    //*cam = transform;
    //cam.translation = transform.translation;
    //cam.translation.x *= -1.0;
    //cam.translation.y *= -1.0;
    //cam.translation.z *= -1.0;
    //cam.rotation = transform.rotation;
    //cam.scale = transform.scale;
    cam.translation = camera_pos;
    //cam.look_to(camera_front, Vec3::Y);
    let back = -camera_front.normalize();
    let up = Vec3::Y;
    let right = up.cross(back).normalize();
    let up = back.cross(right).normalize();
    let rotation_mat = Mat3::from_cols(right, up, back);
    let rotation_quat = Quat::from_mat3(&rotation_mat);
    cam.rotation = rotation_quat;
    //cam.translation.x *= -1.0;

    let top = Vec3::from_array(data.camera_top);
    //println!(
    //    "Pos: {} Front: {} Rotation: {} Top {}",
    //    cam.translation, camera_front, cam.rotation, top
    //);
}

/// sets up a scene with textured entities
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
) {
    let link = GW2Link::new().unwrap();
    let state = GlobalState { gw2link: link };
    commands.spawn(state);
    // load a texture and retrieve its aspect ratio
    let texture_handle = asset_server.load("test.png");
    let aspect = 0.25;

    // create a new quad mesh. this is what we will apply the texture to
    let quad_width = 8.0;
    let quad_handle = meshes.add(Mesh::from(shape::Quad::new(Vec2::new(
        quad_width,
        quad_width * aspect,
    ))));

    // this material renders the texture normally
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // this material modulates the texture to make it red (and slightly transparent)
    let red_material_handle = materials.add(StandardMaterial {
        base_color: Color::rgba(1.0, 0.0, 0.0, 0.5),
        base_color_texture: Some(texture_handle.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // and lets make this one blue! (and also slightly transparent)
    let blue_material_handle = materials.add(StandardMaterial {
        base_color: Color::rgba(0.0, 0.0, 1.0, 0.5),
        base_color_texture: Some(texture_handle),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // textured quad - normal
    commands.spawn(PbrBundle {
        mesh: quad_handle.clone(),
        material: material_handle,
        transform: Transform::from_xyz(0.0, 0.0, 1.5)
            .with_rotation(Quat::from_rotation_x(-PI / 5.0)),
        ..default()
    });
    // textured quad - modulated
    commands.spawn(PbrBundle {
        mesh: quad_handle.clone(),
        material: red_material_handle,
        transform: Transform::from_rotation(Quat::from_rotation_x(-PI / 5.0)),
        ..default()
    });
    // textured quad - modulated
    commands.spawn(PbrBundle {
        mesh: quad_handle,
        material: blue_material_handle,
        transform: Transform::from_xyz(0.0, 1.0, -1.5)
            .with_rotation(Quat::from_rotation_x(-PI / 5.0)),
        ..default()
    });

    let texture_handle = asset_server.load("rust-logo-256x256.png");
    commands.spawn((
        BillboardTextureBundle {
            texture: billboard_textures.add(BillboardTexture::Single(texture_handle.clone())),
            mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(2., 2.)).into())),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        DebugObject,
    ));

    //commands.spawn((BillboardTextureBundle {
    //    texture: billboard_textures.add(BillboardTexture::Single(texture_handle.clone())),
    //    mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(2., 2.)).into())),
    //    transform: Transform::from_xyz(-73.0, 28.0, 211.0),
    //    ..default()
    //},));

    commands.spawn((BillboardTextureBundle {
        texture: billboard_textures.add(BillboardTexture::Single(texture_handle.clone())),
        mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(2., 2.)).into())),
        transform: Transform::from_xyz(-73.0, 28.0, -211.0),
        ..default()
    },));

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
    let mut cam_bundle = Camera3dBundle::default();
    let mut projection = PerspectiveProjection::default();
    projection.fov = 1.222;
    cam_bundle.projection = Projection::Perspective(projection);

    commands.spawn((cam_bundle, Gw2Camera));
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
    obj_query: Query<&Transform, With<DebugObject>>,
    mut text_query: Query<&mut Text, With<DebugText>>,
) {
    let mut text = text_query.single_mut();
    let transform = camera_query.single();
    let obj_transform = obj_query.single();
    text.sections[0].value = format!(
        "X: {:.1} Y: {:.1} Z: {:.1}\nX: {:.1} Y: {:.1} Z: {:.1}",
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        obj_transform.translation.x,
        obj_transform.translation.y,
        obj_transform.translation.z
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

fn load_poi(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut billboard_textures: ResMut<Assets<BillboardTexture>>,
) {
    let data = fs::read_to_string("test.xml").unwrap();

    let mut overlay_data: OverlayData = serde_xml_rs::from_str(&data).unwrap();
    overlay_data.fill_poi_parents();

    overlay_data.pois.poi_list.iter().for_each(|poi| {
        //let tex_file = data.icon_file.unwrap();
        let poi = poi.read().unwrap();
        let texture_handle = asset_server.load(
            poi.get_icon_file()
                .unwrap()
                .to_string_lossy()
                .replace(r"\", "/"),
        );
        commands.spawn((BillboardTextureBundle {
            texture: billboard_textures.add(BillboardTexture::Single(texture_handle.clone())),
            mesh: BillboardMeshHandle(meshes.add(Quad::new(Vec2::new(2., 2.)).into())),
            transform: Transform::from_xyz(poi.pos.xpos, poi.pos.ypos, -poi.pos.zpos),
            ..default()
        },));
    });
}

#[derive(Component)]
struct BevyPOI {
    poi: POI,
}

impl BevyPOI {}
