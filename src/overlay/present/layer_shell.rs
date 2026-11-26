//! The pill on a wlr-layer-shell surface (KDE, COSMIC, Hyprland, niri,
//! Sway). The surface exists only while the pill shows: a mapped surface
//! over a fullscreen game would stop direct scanout.

use super::{pixels, poll, Failure, Pointer, Presenter, Rect, Wake};
use crate::overlay::render::{to_bgra, BOTTOM_MARGIN, SURFACE_HEIGHT, SURFACE_WIDTH};
use anyhow::Context;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::os::fd::BorrowedFd;
use std::time::Duration;
use tiny_skia::Pixmap;
use tracing::debug;
use wayland_client::{
    backend::WaylandError,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, DispatchError, EventQueue, QueueHandle,
};

pub struct LayerShellPresenter {
    queue: EventQueue<State>,
    state: State,
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    pointer: Option<wl_pointer::WlPointer>,
    /// The part of the surface that takes clicks, in logical pixels
    input: Option<(i32, i32, i32, i32)>,
    /// What the pointer did since the last wait
    events: Vec<Pointer>,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: Option<SlotPool>,
    shown: Option<Shown>,
    /// The output scale that the compositor asks for
    scale: i32,
    handle: QueueHandle<State>,
}

struct Shown {
    layer: LayerSurface,
    configured: bool,
    /// A frame callback is outstanding, so the next frame waits for it
    waiting: bool,
    next: Option<Frame>,
}

struct Frame {
    width: u32,
    height: u32,
    scale: i32,
    bgra: Vec<u8>,
}

impl LayerShellPresenter {
    /// `Ok(None)` when the compositor offers no layer-shell.
    pub fn connect() -> anyhow::Result<Option<Self>> {
        let connection = Connection::connect_to_env().context("No Wayland display")?;
        let (globals, queue) = registry_queue_init::<State>(&connection)?;
        let handle = queue.handle();
        let compositor = CompositorState::bind(&globals, &handle)?;
        let Ok(layer_shell) = LayerShell::bind(&globals, &handle) else {
            return Ok(None);
        };
        let shm = Shm::bind(&globals, &handle)?;
        let state = State {
            registry: RegistryState::new(&globals),
            outputs: OutputState::new(&globals, &handle),
            seats: SeatState::new(&globals, &handle),
            pointer: None,
            input: None,
            events: Vec::new(),
            compositor,
            layer_shell,
            shm,
            pool: None,
            shown: None,
            scale: 1,
            handle,
        };
        Ok(Some(Self { queue, state }))
    }
}

impl Presenter for LayerShellPresenter {
    fn name(&self) -> &'static str {
        "Wayland layer-shell"
    }

    fn scale(&self) -> f32 {
        self.state.scale as f32
    }

    fn present(&mut self, frame: Option<&Pixmap>) -> Result<(), Failure> {
        match frame {
            Some(pixmap) => {
                if self.state.shown.is_none() {
                    self.state.show();
                }
                if let Some(shown) = self.state.shown.as_mut() {
                    shown.next = Some(Frame {
                        width: pixmap.width(),
                        height: pixmap.height(),
                        scale: self.state.scale,
                        bgra: to_bgra(pixmap),
                    });
                }
                self.state.draw_next();
            },
            None => self.state.shown = None,
        }
        self.queue.flush().map_err(wayland_failure)
    }

    fn set_input(&mut self, rect: Option<Rect>) -> Result<(), Failure> {
        let input = rect.map(|rect| pixels(rect, 1.0));
        if input != self.state.input {
            self.state.input = input;
            self.state.apply_input();
            self.queue.flush().map_err(wayland_failure)?;
        }
        Ok(())
    }

    fn wait(&mut self, link: BorrowedFd<'_>, timeout: Option<Duration>) -> Result<Wake, Failure> {
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(dispatch_failure)?;
        self.queue.flush().map_err(wayland_failure)?;
        let Some(guard) = self.queue.prepare_read() else {
            // Events wait in the queue: the loop dispatches them first.
            return Ok(Wake {
                link: false,
                pointer: std::mem::take(&mut self.state.events),
            });
        };
        let ready =
            poll(&[guard.connection_fd(), link], timeout).map_err(|e| Failure::Broken(e.into()))?;
        if ready[0] {
            guard.read().map_err(wayland_failure)?;
        } else {
            drop(guard);
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(dispatch_failure)?;
        Ok(Wake {
            link: ready[1],
            pointer: std::mem::take(&mut self.state.events),
        })
    }
}

impl State {
    /// Only the pill takes clicks, and only while a take records; every
    /// other click goes through to the window below.
    fn apply_input(&self) {
        let Some(shown) = self.shown.as_ref() else {
            return;
        };
        if let Ok(region) = Region::new(&self.compositor) {
            if let Some((x, y, width, height)) = self.input {
                region.add(x, y, width, height);
            }
            shown
                .layer
                .wl_surface()
                .set_input_region(Some(region.wl_region()));
            shown.layer.commit();
        }
    }

    fn show(&mut self) {
        let surface = self.compositor.create_surface(&self.handle);
        if let Ok(region) = Region::new(&self.compositor) {
            if let Some((x, y, width, height)) = self.input {
                region.add(x, y, width, height);
            }
            surface.set_input_region(Some(region.wl_region()));
        }
        let layer = self.layer_shell.create_layer_surface(
            &self.handle,
            surface,
            Layer::Overlay,
            Some("hush-overlay"),
            None,
        );
        layer.set_anchor(Anchor::BOTTOM);
        layer.set_margin(0, 0, BOTTOM_MARGIN as i32, 0);
        layer.set_size(SURFACE_WIDTH, SURFACE_HEIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();
        self.shown = Some(Shown {
            layer,
            configured: false,
            waiting: false,
            next: None,
        });
    }

    /// Attach the next frame once the surface is configured and the last
    /// frame was shown.
    fn draw_next(&mut self) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        if !shown.configured || shown.waiting {
            return;
        }
        let Some(frame) = shown.next.take() else {
            return;
        };
        if frame.scale != self.scale {
            // Drawn before a scale change: the loop draws again at once.
            return;
        }
        let pool = match self.pool.as_mut() {
            Some(pool) => pool,
            None => match SlotPool::new(frame.bgra.len() * 2, &self.shm) {
                Ok(pool) => self.pool.insert(pool),
                Err(e) => {
                    debug!("No shared memory for the overlay: {}", e);
                    return;
                },
            },
        };
        let (width, height) = (frame.width as i32, frame.height as i32);
        let Ok((buffer, canvas)) =
            pool.create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)
        else {
            return;
        };
        if canvas.len() != frame.bgra.len() {
            return;
        }
        canvas.copy_from_slice(&frame.bgra);
        let surface = shown.layer.wl_surface();
        surface.set_buffer_scale(frame.scale);
        surface.damage_buffer(0, 0, width, height);
        surface.frame(&self.handle, surface.clone());
        if buffer.attach_to(surface).is_ok() {
            shown.layer.commit();
            shown.waiting = true;
        }
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        factor: i32,
    ) {
        self.scale = factor.max(1);
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        if let Some(shown) = self.shown.as_mut() {
            shown.waiting = false;
        }
        self.draw_next();
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        // The output went away; the next frame makes a new surface.
        self.shown = None;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(shown) = self.shown.as_mut() {
            shown.configured = true;
        }
        self.draw_next();
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seats.get_pointer(handle, &seat).ok();
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

/// linux/input-event-codes.h
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let ours = |event: &PointerEvent| {
            self.shown
                .as_ref()
                .is_some_and(|shown| shown.layer.wl_surface() == &event.surface)
        };
        let seen: Vec<Pointer> = events
            .iter()
            .filter(|event| ours(event))
            .filter_map(|event| match event.kind {
                PointerEventKind::Enter { .. } => Some(Pointer::Enter),
                PointerEventKind::Leave { .. } => Some(Pointer::Leave),
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    Some(Pointer::Primary)
                },
                PointerEventKind::Press { button, .. } if button == BTN_RIGHT => {
                    Some(Pointer::Secondary)
                },
                _ => None,
            })
            .collect();
        self.events.extend(seen);
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(State);
delegate_output!(State);
delegate_shm!(State);
delegate_layer!(State);
delegate_registry!(State);
delegate_seat!(State);
delegate_pointer!(State);

/// A socket error means the compositor went away; anything else is a fault.
fn wayland_failure(error: WaylandError) -> Failure {
    match error {
        WaylandError::Io(_) => Failure::Lost,
        WaylandError::Protocol(error) => {
            Failure::Broken(anyhow::anyhow!("Wayland protocol error: {}", error))
        },
    }
}

fn dispatch_failure(error: DispatchError) -> Failure {
    match error {
        DispatchError::Backend(error) => wayland_failure(error),
        other => Failure::Broken(anyhow::anyhow!("Wayland dispatch failed: {}", other)),
    }
}
