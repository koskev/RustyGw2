use bevy_ecs::{
    entity::Entity,
    event::EventWriter,
    prelude::{Component, Resource},
    system::{Commands, NonSendMut},
    world::Mut,
};
use bevy_utils::{tracing::info, HashMap};
use bevy_window::{RawHandleWrapper, Window, WindowCreated};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use winit::event_loop::EventLoopWindowTarget;

use crate::{converters::convert_winit_theme, WinitWindows};

/// System responsible for creating new windows whenever a [`Window`] component is added
/// to an entity.
///
/// This will default any necessary components if they are not already added.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_window<'a>(
    mut commands: Commands,
    event_loop: &EventLoopWindowTarget<()>,
    created_windows: impl Iterator<Item = (Entity, Mut<'a, Window>)>,
    mut event_writer: EventWriter<WindowCreated>,
    mut winit_windows: NonSendMut<WinitWindows>,
) {
    for (entity, mut window) in created_windows {
        if winit_windows.get_window(entity).is_some() {
            continue;
        }

        info!(
            "Creating new window {:?} ({:?})",
            window.title.as_str(),
            entity
        );

        let winit_window = winit_windows.create_window(event_loop, entity, &window);

        if let Some(theme) = winit_window.theme() {
            window.window_theme = Some(convert_winit_theme(theme));
        }

        //let (my_display, my_window) = custom_window::create_window();

        window
            .resolution
            .set_scale_factor(winit_window.scale_factor());
        commands
            .entity(entity)
            .insert(RawHandleWrapper {
                window_handle: winit_window.raw_window_handle(),
                display_handle: winit_window.raw_display_handle(),
                //window_handle: my_window,
                //display_handle: my_display,
            })
            .insert(CachedWindow {
                window: window.clone(),
            });

        event_writer.send(WindowCreated { window: entity });
    }
}

/// Cache for closing windows so we can get better debug information.
#[derive(Debug, Clone, Resource)]
pub struct WindowTitleCache(HashMap<Entity, String>);

/// The cached state of the window so we can check which properties were changed from within the app.
#[derive(Debug, Clone, Component)]
pub struct CachedWindow {
    pub window: Window,
}
