// SPDX-License-Identifier: GPL-3.0-only

//! Watches the compositor's toplevel list so the applet knows which window
//! has focus, and activates windows on request. Trimmed down from the
//! minimize applet's handler (no screencopy needed here).

use crate::wayland_subscription::{ToplevelRequest, ToplevelUpdate, WaylandRequest, WaylandUpdate};
use std::{
    collections::HashMap,
    os::{
        fd::{FromRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use cctk::{
    sctk::{
        self,
        reexports::{calloop, calloop_wayland_source::WaylandSource},
        seat::{SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{self, WEnum, protocol::wl_seat::WlSeat},
};
use cosmic::{
    cctk::{
        self,
        cosmic_protocols::{
            self, toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        },
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    },
    iced::futures,
};
use cosmic_protocols::{
    toplevel_info::v1::client::zcosmic_toplevel_handle_v1,
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
use futures::{SinkExt, channel::mpsc};
use sctk::registry::{ProvidesRegistryState, RegistryState};
use wayland_client::{Connection, QueueHandle, globals::registry_queue_init};

struct AppData {
    exit: bool,
    tx: mpsc::Sender<WaylandUpdate>,
    registry_state: RegistryState,
    toplevel_info_state: ToplevelInfoState,
    toplevel_manager_state: ToplevelManagerState,
    seat_state: SeatState,
    /// Last `(app_id, activated)` forwarded per window. Windows emit an update
    /// for every title change; this keeps those from reaching the applet.
    last_sent: HashMap<ExtForeignToplevelHandleV1, (String, bool)>,
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    sctk::registry_handlers!();
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut sctk::seat::SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: WlSeat,
        _: sctk::seat::Capability,
    ) {
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: WlSeat,
        _: sctk::seat::Capability,
    ) {
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl ToplevelManagerHandler for AppData {
    fn toplevel_manager_state(&mut self) -> &mut cctk::toplevel_management::ToplevelManagerState {
        &mut self.toplevel_manager_state
    }

    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
    }
}

impl AppData {
    fn cosmic_toplevel(
        &self,
        handle: &ExtForeignToplevelHandleV1,
    ) -> Option<ZcosmicToplevelHandleV1> {
        self.toplevel_info_state
            .info(handle)?
            .cosmic_toplevel
            .clone()
    }

    fn handle_toplevel(&mut self, toplevel: &ExtForeignToplevelHandleV1) {
        let Some(info) = self.toplevel_info_state.info(toplevel) else {
            return;
        };
        let state = (
            info.app_id.clone(),
            info.state
                .contains(&zcosmic_toplevel_handle_v1::State::Activated),
        );

        if self.last_sent.get(toplevel) == Some(&state) {
            return;
        }
        self.last_sent.insert(toplevel.clone(), state.clone());

        let update = ToplevelUpdate::Changed {
            handle: toplevel.clone(),
            app_id: state.0,
            activated: state.1,
        };
        let _ = futures::executor::block_on(self.tx.send(WaylandUpdate::Toplevel(update)));
    }

    fn remove_toplevel(&mut self, toplevel: &ExtForeignToplevelHandleV1) {
        self.last_sent.remove(toplevel);
        let _ = futures::executor::block_on(self.tx.send(WaylandUpdate::Toplevel(
            ToplevelUpdate::Removed(toplevel.clone()),
        )));
    }
}

impl ToplevelInfoHandler for AppData {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.toplevel_info_state
    }

    fn new_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        self.handle_toplevel(toplevel);
    }

    fn update_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        self.handle_toplevel(toplevel);
    }

    fn toplevel_closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        self.remove_toplevel(toplevel);
    }
}

pub(crate) fn wayland_handler(
    tx: mpsc::Sender<WaylandUpdate>,
    rx: calloop::channel::Channel<WaylandRequest>,
) {
    let socket = std::env::var("X_PRIVILEGED_WAYLAND_SOCKET")
        .ok()
        .and_then(|fd| {
            fd.parse::<RawFd>()
                .ok()
                .map(|fd| unsafe { UnixStream::from_raw_fd(fd) })
        });

    let conn = if let Some(socket) = socket {
        Connection::from_socket(socket).unwrap()
    } else {
        Connection::connect_to_env().unwrap()
    };
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();

    let mut event_loop = calloop::EventLoop::<AppData>::try_new().unwrap();
    let qh = event_queue.handle();
    let wayland_source = WaylandSource::new(conn.clone(), event_queue);
    let handle = event_loop.handle();
    wayland_source
        .insert(handle.clone())
        .expect("Failed to insert wayland source.");

    if handle
        .insert_source(rx, |event, (), state| match event {
            calloop::channel::Event::Msg(req) => match req {
                WaylandRequest::Toplevel(req) => match req {
                    ToplevelRequest::Activate(handle) => {
                        if let Some(seat) = state.seat_state.seats().next() {
                            let manager = &state.toplevel_manager_state.manager;
                            if let Some(cosmic_toplevel) = state.cosmic_toplevel(&handle) {
                                manager.activate(&cosmic_toplevel, &seat);
                            }
                        }
                    }
                },
            },
            calloop::channel::Event::Closed => {
                state.exit = true;
            }
        })
        .is_err()
    {
        return;
    }
    let registry_state = RegistryState::new(&globals);

    let mut app_data = AppData {
        exit: false,
        tx,
        seat_state: SeatState::new(&globals, &qh),
        toplevel_info_state: ToplevelInfoState::new(&registry_state, &qh),
        toplevel_manager_state: ToplevelManagerState::new(&registry_state, &qh),
        registry_state,
        last_sent: HashMap::new(),
    };

    loop {
        if app_data.exit {
            break;
        }
        event_loop.dispatch(None, &mut app_data).unwrap();
    }
}

sctk::delegate_seat!(AppData);
sctk::delegate_registry!(AppData);
cctk::delegate_toplevel_info!(AppData);
cctk::delegate_toplevel_manager!(AppData);
