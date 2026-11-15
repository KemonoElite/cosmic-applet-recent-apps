// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    cctk::{
        sctk::reexports::calloop,
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    },
    iced::{self, Subscription},
    iced::{futures, stream},
};
use futures::SinkExt;

use crate::wayland_handler::wayland_handler;

pub fn wayland_subscription() -> iced::Subscription<WaylandUpdate> {
    Subscription::run_with(std::any::TypeId::of::<WaylandUpdate>(), |_| {
        stream::channel(
            1,
            move |mut output: futures::channel::mpsc::Sender<WaylandUpdate>| async move {
                let (calloop_tx, calloop_rx) = calloop::channel::channel();
                let runtime = tokio::runtime::Handle::current();

                let _ = std::thread::spawn(move || {
                    runtime.block_on(async move {
                        _ = output.send(WaylandUpdate::Init(calloop_tx)).await;
                        wayland_handler(output.clone(), calloop_rx);
                        tracing::error!("Wayland handler thread died");
                        _ = output.send(WaylandUpdate::Finished).await;
                    });
                });

                futures::future::pending().await
            },
        )
    })
}

#[derive(Clone, Debug)]
pub enum WaylandUpdate {
    Init(calloop::channel::Sender<WaylandRequest>),
    Finished,
    Toplevel(ToplevelUpdate),
}

#[derive(Clone, Debug)]
pub enum ToplevelUpdate {
    /// A window appeared, or its app id / focus state changed.
    Changed {
        handle: ExtForeignToplevelHandleV1,
        app_id: String,
        activated: bool,
    },
    Removed(ExtForeignToplevelHandleV1),
}

#[derive(Clone, Debug)]
pub enum WaylandRequest {
    Toplevel(ToplevelRequest),
}

#[derive(Debug, Clone)]
pub enum ToplevelRequest {
    Activate(ExtForeignToplevelHandleV1),
}
