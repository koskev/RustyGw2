//! This example shows various ways to configure texture materials in 3D.

use bevy_utils::Duration;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle,
};
use x11::{
    xfixes::{XFixesCreateRegion, XFixesDestroyRegion, XFixesSetWindowShapeRegion},
    xlib::{
        CWBackPixmap, CWBorderPixel, CWColormap, CWEventMask, InputOutput, NoEventMask, TrueColor,
        Visual, XCreateGC, XCreateWindow, XDefaultRootWindow, XDefaultScreen, XGCValues,
        XMatchVisualInfo, XOpenDisplay, XRectangle, XSetWindowAttributes, XVisualInfo, GC,
    },
    xlib_xcb::XGetXCBConnection,
};
use xcb::{
    shape::Sk,
    x::{self, Colormap, CreateColormap, CwMask, EventMask, Rectangle, VisualClass, Visualtype},
    xfixes::{self, Region},
    Xid,
};

use std::{sync::Arc, thread};

pub fn create_window() -> (RawDisplayHandle, RawWindowHandle) {
    let x = 1680;
    let y = 0;
    let w = 1920;
    let h = 1080;

    let mut base_event_mask = EventMask::empty();
    //base_event_mask.set(EventMask::EXPOSURE, true);
    //base_event_mask.set(EventMask::STRUCTURE_NOTIFY, true);
    //base_event_mask.set(EventMask::PROPERTY_CHANGE, true);
    //base_event_mask.set(EventMask::FOCUS_CHANGE, true);

    let mut transparent_input_mask = EventMask::from(base_event_mask);
    //transparent_input_mask.set(EventMask::VISIBILITY_CHANGE, true);
    //transparent_input_mask.set(EventMask::RESIZE_REDIRECT, true);
    ////transparent_input_mask.set(EventMask::SUBSTRUCTURE_REDIRECT, true);
    //transparent_input_mask.set(EventMask::COLOR_MAP_CHANGE, true);
    //transparent_input_mask.set(EventMask::OWNER_GRAB_BUTTON, true);

    //let mut cw_mask = CwMask::empty();
    //cw_mask.set(CwMask::OVERRIDE_REDIRECT, true);
    //cw_mask.set(CwMask::EVENT_MASK, true);

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let conn = Arc::new(conn);

    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();

    let depth = screen
        .allowed_depths()
        .find(|d| d.depth() == 32 && d.visuals().len() > 0)
        .unwrap();

    let visual = depth
        .visuals()
        .iter()
        .find(|v| v.class() == VisualClass::TrueColor)
        .unwrap();

    let colormap_id = conn.generate_id();

    let cookie = conn.send_request_checked(&x::CreateColormap {
        alloc: x::ColormapAlloc::None,
        mid: colormap_id,
        window: screen.root(),
        visual: visual.visual_id(),
    });
    conn.check_request(cookie).unwrap();

    println!(
        "Colormap id: {:?} visual id {:?}",
        colormap_id,
        visual.visual_id()
    );

    let window = conn.generate_id();
    let cookie = conn.send_request_checked(&x::CreateWindow {
        depth: depth.depth(),
        wid: window,
        parent: screen.root(),
        x,
        y,
        width: w,
        height: h,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: visual.visual_id(),
        // this list must be in same order than `Cw` enum order
        value_list: &[
            x::Cw::BackPixmap(x::BACKPIXMAP_NONE),
            x::Cw::BackPixel(0x808080),
            x::Cw::BorderPixel(0),
            x::Cw::OverrideRedirect(true),
            x::Cw::EventMask(transparent_input_mask),
            x::Cw::Colormap(colormap_id),
        ],
    });
    conn.check_request(cookie).unwrap();

    //let region_id = conn.generate_id();
    //println!("region id: {:?}", region_id);
    //let rectangle = Rectangle {
    //    x: w as i16,
    //    y: 0,
    //    width: w + x as u16,
    //    height: h,
    //};

    //let cookie = conn.send_request_checked(&xfixes::CreateRegion {
    //    region: region_id,
    //    rectangles: &[rectangle],
    //});
    ////conn.check_request(cookie).unwrap();
    ////let cookie = conn.send_request_checked(&xfixes::SetWindowShapeRegion {
    ////    dest: window,
    ////    dest_kind: Sk::Input,
    ////    x_offset: 0,
    ////    y_offset: 0,
    ////    region: region_id,
    ////});
    ////conn.check_request(cookie).unwrap();

    //let cookie = conn.send_request_checked(&xfixes::DestroyRegion { region: region_id });
    //conn.check_request(cookie).unwrap();

    // We now show ("map" in X terminology) the window.
    // This time we do not check for success, so we discard the cookie.
    conn.send_request(&x::MapWindow { window });

    let mut display_handle = XcbDisplayHandle::empty();
    display_handle.connection = conn.get_raw_conn() as *mut _;
    display_handle.screen = screen_num as _;

    let mut window_handle = XcbWindowHandle::empty();
    window_handle.window = window.resource_id() as _;
    window_handle.visual_id = screen.root_visual() as _;

    conn.flush().unwrap();

    {
        let conn = conn.clone();
        thread::spawn(move || loop {
            let event = conn.wait_for_event().unwrap();
            println!("Event: {:?}", event);
        });
    }

    (
        RawDisplayHandle::Xcb(display_handle),
        RawWindowHandle::Xcb(window_handle),
    )
}
