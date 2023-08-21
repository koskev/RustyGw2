#![allow(clippy::type_complexity)]
#![warn(missing_docs)]
//! `bevy_winit` provides utilities to handle window creation and the eventloop through [`winit`]
//!
//! Most commonly, the [`WinitPlugin`] is used as part of
//! [`DefaultPlugins`](https://docs.rs/bevy/latest/bevy/struct.DefaultPlugins.html).
//! The app's [runner](bevy_app::App::runner) is set by `WinitPlugin` and handles the `winit` [`EventLoop`](winit::event_loop::EventLoop).
//! See `winit_runner` for details.

pub mod accessibility;
mod converters;
mod custom_window;
mod system;
mod winit_config;
mod winit_windows;

use bevy_ecs::system::SystemState;
use bevy_tasks::tick_global_task_pools_on_main_thread;
use system::{changed_window, create_window, despawn_window, CachedWindow};

pub use winit_config::*;
pub use winit_windows::*;

use bevy_app::{App, AppExit, Last, Plugin};
use bevy_ecs::event::{Events, ManualEventReader};
use bevy_ecs::prelude::*;
use bevy_input::mouse::MouseMotion;
use bevy_math::Vec2;
use bevy_utils::{
    tracing::{trace, warn},
    Instant,
};
use bevy_window::{exit_on_all_closed, RequestRedraw, Window, WindowCreated};

use winit::{
    event::{self, DeviceEvent, Event, StartCause},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopWindowTarget},
};

use crate::accessibility::AccessibilityPlugin;

/// A [`Plugin`] that utilizes [`winit`] for window creation and event loop management.
#[derive(Default)]
pub struct WinitPlugin;

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) {
        let mut event_loop_builder = EventLoopBuilder::<()>::with_user_event();

        let event_loop = event_loop_builder.build();
        app.insert_non_send_resource(event_loop);

        app.init_non_send_resource::<WinitWindows>()
            .init_resource::<WinitSettings>()
            .set_runner(winit_runner)
            // exit_on_all_closed only uses the query to determine if the query is empty,
            // and so doesn't care about ordering relative to changed_window
            .add_systems(
                Last,
                (
                    changed_window.ambiguous_with(exit_on_all_closed),
                    // Update the state of the window before attempting to despawn to ensure consistent event ordering
                    despawn_window.after(changed_window),
                ),
            );

        app.add_plugins(AccessibilityPlugin);

        let mut create_window_system_state: SystemState<(
            Commands,
            NonSendMut<EventLoop<()>>,
            Query<(Entity, &mut Window)>,
            EventWriter<WindowCreated>,
            NonSendMut<WinitWindows>,
        )> = SystemState::from_world(&mut app.world);

        {
            let (commands, event_loop, mut new_windows, event_writer, winit_windows) =
                create_window_system_state.get_mut(&mut app.world);

            // Here we need to create a winit-window and give it a WindowHandle which the renderer can use.
            // It needs to be spawned before the start of the startup schedule, so we cannot use a regular system.
            // Instead we need to create the window and spawn it using direct world access
            create_window(
                commands,
                &event_loop,
                new_windows.iter_mut(),
                event_writer,
                winit_windows,
            );
        }

        create_window_system_state.apply(&mut app.world);
    }
}

fn run<F>(event_loop: EventLoop<()>, event_handler: F) -> !
where
    F: 'static + FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow),
{
    event_loop.run(event_handler)
}

fn run_return<F>(event_loop: &mut EventLoop<()>, event_handler: F)
where
    F: FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow),
{
    use winit::platform::run_return::EventLoopExtRunReturn;
    event_loop.run_return(event_handler);
}

/// Stores state that must persist between frames.
struct WinitPersistentState {
    /// Tracks whether or not the application is active or suspended.
    active: bool,
    /// Tracks whether or not an event has occurred this frame that would trigger an update in low
    /// power mode. Should be reset at the end of every frame.
    low_power_event: bool,
    /// Tracks whether the event loop was started this frame because of a redraw request.
    redraw_request_sent: bool,
    /// Tracks if the event loop was started this frame because of a [`ControlFlow::WaitUntil`]
    /// timeout.
    timeout_reached: bool,
    last_update: Instant,
}
impl Default for WinitPersistentState {
    fn default() -> Self {
        Self {
            active: false,
            low_power_event: false,
            redraw_request_sent: false,
            timeout_reached: false,
            last_update: Instant::now(),
        }
    }
}

/// The default [`App::runner`] for the [`WinitPlugin`] plugin.
///
/// Overriding the app's [runner](bevy_app::App::runner) while using `WinitPlugin` will bypass the `EventLoop`.
pub fn winit_runner(mut app: App) {
    // We remove this so that we have ownership over it.
    let mut event_loop = app
        .world
        .remove_non_send_resource::<EventLoop<()>>()
        .unwrap();

    let mut app_exit_event_reader = ManualEventReader::<AppExit>::default();
    let mut redraw_event_reader = ManualEventReader::<RequestRedraw>::default();
    let mut winit_state = WinitPersistentState::default();
    app.world
        .insert_non_send_resource(event_loop.create_proxy());

    let return_from_run = app.world.resource::<WinitSettings>().return_from_run;

    trace!("Entering winit event loop");

    let mut focused_window_state: SystemState<(Res<WinitSettings>, Query<&Window>)> =
        SystemState::from_world(&mut app.world);

    let mut create_window_system_state: SystemState<(
        Commands,
        Query<(Entity, &mut Window), Added<Window>>,
        EventWriter<WindowCreated>,
        NonSendMut<WinitWindows>,
    )> = SystemState::from_world(&mut app.world);

    let mut finished_and_setup_done = false;

    let event_handler = move |event: Event<()>,
                              event_loop: &EventLoopWindowTarget<()>,
                              control_flow: &mut ControlFlow| {
        #[cfg(feature = "trace")]
        let _span = bevy_utils::tracing::info_span!("winit event_handler").entered();

        if !finished_and_setup_done {
            if !app.ready() {
                tick_global_task_pools_on_main_thread();
            } else {
                app.finish();
                app.cleanup();
                finished_and_setup_done = true;
            }
        }

        if let Some(app_exit_events) = app.world.get_resource::<Events<AppExit>>() {
            if app_exit_event_reader.iter(app_exit_events).last().is_some() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        match event {
            event::Event::NewEvents(start) => {
                let (winit_config, window_focused_query) = focused_window_state.get(&app.world);

                let app_focused = window_focused_query.iter().any(|window| window.focused);

                // Check if either the `WaitUntil` timeout was triggered by winit, or that same
                // amount of time has elapsed since the last app update. This manual check is needed
                // because we don't know if the criteria for an app update were met until the end of
                // the frame.
                let auto_timeout_reached = matches!(start, StartCause::ResumeTimeReached { .. });
                let now = Instant::now();
                let manual_timeout_reached = match winit_config.update_mode(app_focused) {
                    UpdateMode::Continuous => false,
                    UpdateMode::Reactive { max_wait }
                    | UpdateMode::ReactiveLowPower { max_wait } => {
                        now.duration_since(winit_state.last_update) >= *max_wait
                    }
                };
                // The low_power_event state and timeout must be reset at the start of every frame.
                winit_state.low_power_event = false;
                winit_state.timeout_reached = auto_timeout_reached || manual_timeout_reached;
            }
            event::Event::WindowEvent {
                event,
                window_id: winit_window_id,
                ..
            } => {
                // Fetch and prepare details from the world
                let mut system_state: SystemState<(
                    NonSend<WinitWindows>,
                    Query<(&mut Window, &mut CachedWindow)>,
                )> = SystemState::new(&mut app.world);
                let (winit_windows, mut window_query) = system_state.get_mut(&mut app.world);

                // Entity of this window
                let window_entity =
                    if let Some(entity) = winit_windows.get_window_entity(winit_window_id) {
                        entity
                    } else {
                        warn!(
                            "Skipped event {:?} for unknown winit Window Id {:?}",
                            event, winit_window_id
                        );
                        return;
                    };

                let (window, mut cache) =
                    if let Ok((window, info)) = window_query.get_mut(window_entity) {
                        (window, info)
                    } else {
                        warn!(
                            "Window {:?} is missing `Window` component, skipping event {:?}",
                            window_entity, event
                        );
                        return;
                    };

                winit_state.low_power_event = true;

                if window.is_changed() {
                    cache.window = window.clone();
                }
            }
            event::Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (x, y) },
                ..
            } => {
                let mut system_state: SystemState<EventWriter<MouseMotion>> =
                    SystemState::new(&mut app.world);
                let mut mouse_motion = system_state.get_mut(&mut app.world);

                mouse_motion.send(MouseMotion {
                    delta: Vec2::new(x as f32, y as f32),
                });
            }
            event::Event::Suspended => {
                winit_state.active = false;
            }
            event::Event::Resumed => {
                winit_state.active = true;
            }
            event::Event::MainEventsCleared => {
                let (winit_config, window_focused_query) = focused_window_state.get(&app.world);

                let update = if winit_state.active {
                    // True if _any_ windows are currently being focused
                    let app_focused = window_focused_query.iter().any(|window| window.focused);
                    match winit_config.update_mode(app_focused) {
                        UpdateMode::Continuous | UpdateMode::Reactive { .. } => true,
                        UpdateMode::ReactiveLowPower { .. } => {
                            winit_state.low_power_event
                                || winit_state.redraw_request_sent
                                || winit_state.timeout_reached
                        }
                    }
                } else {
                    false
                };

                if update && finished_and_setup_done {
                    winit_state.last_update = Instant::now();
                    app.update();
                }
            }
            Event::RedrawEventsCleared => {
                {
                    // Fetch from world
                    let (winit_config, window_focused_query) = focused_window_state.get(&app.world);

                    // True if _any_ windows are currently being focused
                    let app_focused = window_focused_query.iter().any(|window| window.focused);

                    let now = Instant::now();
                    use UpdateMode::*;
                    *control_flow = match winit_config.update_mode(app_focused) {
                        Continuous => ControlFlow::Poll,
                        Reactive { max_wait } | ReactiveLowPower { max_wait } => {
                            if let Some(instant) = now.checked_add(*max_wait) {
                                ControlFlow::WaitUntil(instant)
                            } else {
                                ControlFlow::Wait
                            }
                        }
                    };
                }

                // This block needs to run after `app.update()` in `MainEventsCleared`. Otherwise,
                // we won't be able to see redraw requests until the next event, defeating the
                // purpose of a redraw request!
                let mut redraw = false;
                if let Some(app_redraw_events) = app.world.get_resource::<Events<RequestRedraw>>() {
                    if redraw_event_reader.iter(app_redraw_events).last().is_some() {
                        *control_flow = ControlFlow::Poll;
                        redraw = true;
                    }
                }

                winit_state.redraw_request_sent = redraw;
            }

            _ => (),
        }

        if winit_state.active {
            let (commands, mut new_windows, created_window_writer, winit_windows) =
                create_window_system_state.get_mut(&mut app.world);

            // Responsible for creating new windows
            create_window(
                commands,
                event_loop,
                new_windows.iter_mut(),
                created_window_writer,
                winit_windows,
            );

            create_window_system_state.apply(&mut app.world);
        }
    };

    // If true, returns control from Winit back to the main Bevy loop
    if return_from_run {
        run_return(&mut event_loop, event_handler);
    } else {
        run(event_loop, event_handler);
    }
}
