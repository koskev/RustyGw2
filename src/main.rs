//! This example shows various ways to configure texture materials in 3D.

use std::{f32::consts::PI, ptr::null};

use bevy::{
    prelude::*,
    window::{Window, WindowLevel, WindowResolution},
};

mod gw2link;
mod processutils;

use gw2link::GW2Link;
use x11::{
    xlib::{
        CWBackPixmap, CWBorderPixel, CWColormap, CWEventMask, InputOutput, NoEventMask, TrueColor,
        Visual, XCreateGC, XCreateWindow, XDefaultRootWindow, XDefaultScreen, XGCValues,
        XMatchVisualInfo, XOpenDisplay, XSetWindowAttributes, XVisualInfo, GC,
    },
    xlib_xcb::XGetXCBConnection,
};
use xcb::x::{self, CwMask, EventMask};

#[derive(Component)]
struct GlobalState {
    gw2link: GW2Link,
}

fn create_window() -> Window {
    let x = 1;
    let y = 1;
    let w = 0;
    let h = 0;

    //let display = unsafe { XOpenDisplay(null()) };

    //let visual: *mut Visual = null::<Visual>() as *mut Visual;
    //let mut visual_info: XVisualInfo = XVisualInfo {
    //    visual,
    //    visualid: 0,
    //    screen: 0,
    //    depth: 0,
    //    class: 0,
    //    red_mask: 0,
    //    green_mask: 0,
    //    blue_mask: 0,
    //    colormap_size: 0,
    //    bits_per_rgb: 0,
    //};
    //unsafe {
    //    XMatchVisualInfo(
    //        display,
    //        XDefaultScreen(display),
    //        32,
    //        TrueColor,
    //        &mut visual_info,
    //    );
    //}

    //let mut attr: XSetWindowAttributes = XSetWindowAttributes {
    //    background_pixmap: 0, // "None" in my C code
    //    background_pixel: 0,
    //    border_pixmap: 0,
    //    border_pixel: 0,
    //    bit_gravity: 0,
    //    win_gravity: 0,
    //    backing_store: 0,
    //    backing_planes: 0,
    //    backing_pixel: 0,
    //    save_under: 0,
    //    event_mask: NoEventMask,
    //    do_not_propagate_mask: 0,
    //    override_redirect: 0,
    //    colormap: 0,
    //    cursor: 0,
    //};

    //let window = unsafe {
    //    XCreateWindow(
    //        display,
    //        XDefaultRootWindow(display),
    //        x,
    //        y,
    //        w,
    //        h,
    //        0,
    //        visual_info.depth,
    //        InputOutput as u32,
    //        visual_info.visual,
    //        CWColormap | CWEventMask | CWBackPixmap | CWBorderPixel,
    //        &mut attr,
    //    )
    //};
    //let gc = unsafe { XCreateGC(display, window, 0, null::<XGCValues>() as *mut XGCValues) };
    let mut base_event_mask = EventMask::empty();
    base_event_mask.set(EventMask::EXPOSURE, true);
    base_event_mask.set(EventMask::STRUCTURE_NOTIFY, true);
    base_event_mask.set(EventMask::PROPERTY_CHANGE, true);
    base_event_mask.set(EventMask::FOCUS_CHANGE, true);

    let mut transparent_input_mask = EventMask::from(base_event_mask);
    transparent_input_mask.set(EventMask::VISIBILITY_CHANGE, true);
    transparent_input_mask.set(EventMask::RESIZE_REDIRECT, true);
    transparent_input_mask.set(EventMask::SUBSTRUCTURE_REDIRECT, true);
    transparent_input_mask.set(EventMask::COLOR_MAP_CHANGE, true);
    transparent_input_mask.set(EventMask::OWNER_GRAB_BUTTON, true);

    let mut cw_mask = CwMask::empty();
    cw_mask.set(CwMask::OVERRIDE_REDIRECT, true);
    cw_mask.set(CwMask::EVENT_MASK, true);

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();

    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();
    let window: x::Window = conn.generate_id();

    let cookie = conn.send_request_checked(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width: 150,
        height: 150,
        border_width: 5,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        // this list must be in same order than `Cw` enum order
        value_list: &[
            x::Cw::BackPixmap(x::BACKPIXMAP_NONE),
            //x::Cw::BackPixel(screen.black_pixel()),
            x::Cw::BorderPixel(0),
            x::Cw::EventMask(transparent_input_mask),
            x::Cw::Colormap(screen.default_colormap()),
        ],
    });
    conn.check_request(cookie).unwrap();

    // We now show ("map" in X terminology) the window.
    // This time we do not check for success, so we discard the cookie.
    conn.send_request(&x::MapWindow { window });

    conn.flush().unwrap();

    loop {
        conn.wait_for_event().unwrap();
    }
    let w = Window::default();
}

fn main() {
    create_window();
    loop {}
    return ();
    let mut window_descriptor = Window {
        // Enable transparent support for the window
        transparent: true,
        decorations: false,
        window_level: WindowLevel::AlwaysOnTop,
        // Allows inputs to pass through to apps behind this app. New to bevy 0.10
        resolution: WindowResolution::new(800.0, 600.0),
        ..default()
    };
    window_descriptor.cursor.hit_test = false;

    let pid = processutils::find_wine_process("GW2-64.exe");
    println!("Got pid {:?}", pid);
    processutils::start_gw2_helper(pid.unwrap(), "/tmp/mumble.exe");

    App::new()
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
        .add_systems(Update, update_gw2)
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(window_descriptor),
            ..default()
        }))
        .run();
}

fn update_gw2(mut query: Query<&mut GlobalState>, _time: Res<Time>) {
    query.single_mut().gw2link.update_gw2(false);
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
        transform: Transform::from_xyz(0.0, 0.0, -1.5)
            .with_rotation(Quat::from_rotation_x(-PI / 5.0)),
        ..default()
    });
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(3.0, 5.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn rotate_camera(mut query: Query<&mut Transform, With<Camera>>, time: Res<Time>) {
    let mut transform = query.single_mut();

    transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(time.delta_seconds() / 2.));
}
