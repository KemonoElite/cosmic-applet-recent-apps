// SPDX-License-Identifier: GPL-3.0-only

//! Panel applet showing shortcuts to the most recently used apps.
//!
//! "Used" means "had a window focused". Focus changes come from the
//! compositor's toplevel list; the ordered list of app ids is persisted with
//! cosmic-config so it survives restarts and is shared between panel instances
//! (one applet process runs per output).
//!
//! Clicking an icon focuses that app's window if it is open, and launches it
//! otherwise.

mod config;
mod localize;
pub(crate) mod wayland_handler;
pub(crate) mod wayland_subscription;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
};

use config::{APP_ID, MAX_SHOWN, RecentAppsConfig, TOOLKIT_CONFIG_ID, ToolkitIconTheme};
use cosmic::{
    Element, Task, app,
    cctk::{
        sctk::reexports::calloop,
        wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    },
    cosmic_config::{Config, ConfigGet, CosmicConfigEntry},
    cosmic_theme::{Theme as CosmicTheme, ThemeMode},
    desktop::{IconSourceExt, fde},
    iced::{
        self, Alignment, Length, Limits, Subscription,
        id::Id as WidgetId,
        widget::{Column, Row},
    },
    surface,
    widget::autosize::autosize,
};
use wayland_subscription::{
    ToplevelRequest, ToplevelUpdate, WaylandRequest, WaylandUpdate, wayland_subscription,
};

use crate::localize::localize;

static AUTOSIZE_MAIN_ID: LazyLock<WidgetId> = LazyLock::new(|| WidgetId::new("autosize-main"));

/// Icon theme from the dark/light theme config, whichever mode is active
/// (where newer COSMIC Settings builds store it).
fn theme_config_icon_theme() -> Option<String> {
    let mode = ThemeMode::config().ok()?;
    let is_dark = ThemeMode::is_dark(&mode).ok()?;
    let config = if is_dark {
        CosmicTheme::dark_config()
    } else {
        CosmicTheme::light_config()
    }
    .ok()?;
    config.get::<String>("icon_theme").ok()
}

/// Icon theme from the toolkit config (where released COSMIC Settings store it).
fn toolkit_config_icon_theme() -> Option<String> {
    let config = Config::new(TOOLKIT_CONFIG_ID, ToolkitIconTheme::VERSION).ok()?;
    config.get::<String>("icon_theme").ok()
}

/// The icon theme chosen in COSMIC Settings > Desktop > Appearance > Icon theme.
fn desktop_icon_theme() -> Option<(String, &'static str)> {
    theme_config_icon_theme()
        .map(|name| (name, "theme config"))
        .or_else(|| toolkit_config_icon_theme().map(|name| (name, "toolkit config")))
}

/// Points the toolkit's icon lookups at the desktop's icon theme.
fn follow_desktop_icon_theme() {
    match desktop_icon_theme() {
        Some((name, source)) => {
            if cosmic::icon_theme::default() != name {
                tracing::info!(icon_theme = %name, source, "using the desktop's icon theme");
                cosmic::icon_theme::set_default(name);
            }
        }
        None => {
            tracing::warn!("could not read the icon theme from the COSMIC theme or toolkit config")
        }
    }
}

pub fn run() -> cosmic::iced::Result {
    localize();
    cosmic::applet::run::<RecentApps>(())
}

/// One icon in the panel.
struct Item {
    app_id: String,
    name: String,
    icon_source: fde::IconSource,
    entry: Arc<fde::DesktopEntry>,
}

#[derive(Default)]
struct RecentApps {
    core: cosmic::app::Core,
    locales: Vec<String>,
    desktop_entries: Vec<fde::DesktopEntry>,
    /// App ids we already failed to resolve to a desktop entry, so that a
    /// window without one doesn't trigger a rescan on every title change.
    unresolvable: HashSet<String>,
    config: RecentAppsConfig,
    config_helper: Option<Config>,
    /// What is currently drawn: the first `MAX_SHOWN` recent apps that resolve.
    items: Vec<Item>,
    /// Open windows and their app ids.
    toplevels: Vec<(ExtForeignToplevelHandleV1, String)>,
    /// The window of each app that was focused most recently.
    last_focused: HashMap<String, ExtForeignToplevelHandleV1>,
    tx: Option<calloop::channel::Sender<WaylandRequest>>,
    zbus: Option<zbus::Connection>,
}

#[derive(Debug, Clone)]
enum Message {
    Wayland(WaylandUpdate),
    ConfigUpdated(RecentAppsConfig),
    IconThemeChanged,
    Open(String),
    Surface(surface::Action<Message>),
}

impl RecentApps {
    fn update_desktop_entries(&mut self) {
        self.desktop_entries = fde::Iter::new(fde::default_paths())
            .filter_map(|p| fde::DesktopEntry::from_path(p, Some(&self.locales)).ok())
            .collect();
        self.unresolvable.clear();
    }

    fn find_entry(&self, app_id: &str) -> Option<&fde::DesktopEntry> {
        fde::find_app_by_id(&self.desktop_entries, fde::unicase::Ascii::new(app_id))
    }

    /// Rebuilds the drawn items from the saved list and the cached desktop entries.
    fn rebuild_items(&mut self) {
        let mut items: Vec<Item> = Vec::with_capacity(MAX_SHOWN);

        for app_id in &self.config.recent {
            if items.len() == MAX_SHOWN {
                break;
            }
            // Never list ourselves (can be saved if the applet was run as a normal window).
            if app_id == APP_ID {
                continue;
            }
            let Some(entry) = self.find_entry(app_id) else {
                continue;
            };
            // Two window ids can map to one desktop entry.
            if items.iter().any(|i| i.entry.appid == entry.appid) {
                continue;
            }

            let icon_name = entry.icon().unwrap_or(&entry.appid);
            tracing::info!(
                app_id = %app_id,
                desktop_entry = %entry.appid,
                icon = icon_name,
                icon_theme = %cosmic::icon_theme::default(),
                resolved = ?cosmic::widget::icon::from_name(icon_name)
                    .prefer_svg(true)
                    .size(128)
                    .path(),
                "recent app icon"
            );

            items.push(Item {
                app_id: app_id.clone(),
                name: entry
                    .full_name(&self.locales)
                    .unwrap_or(Cow::Borrowed(&entry.appid))
                    .into_owned(),
                icon_source: fde::IconSource::from_unknown(entry.icon().unwrap_or(&entry.appid)),
                entry: Arc::new(entry.clone()),
            });
        }

        self.items = items;
    }

    /// Marks `app_id` as the most recently used app.
    fn record_use(&mut self, app_id: &str) {
        if app_id == APP_ID {
            return;
        }
        if self
            .config
            .recent
            .first()
            .is_some_and(|first| first == app_id)
            || self.unresolvable.contains(app_id)
        {
            return;
        }

        // Only remember apps we can show an icon for and launch again.
        if self.find_entry(app_id).is_none() {
            // Might be a freshly installed app.
            self.update_desktop_entries();
            if self.find_entry(app_id).is_none() {
                tracing::warn!(app_id, "no desktop entry found for window, not tracking it");
                self.unresolvable.insert(app_id.to_owned());
                return;
            }
        }

        if self.config.touch(app_id) {
            tracing::info!(app_id, "most recently used app");
            if let Some(helper) = &self.config_helper {
                self.config.save(helper);
            }
            self.rebuild_items();
        }
    }

    /// The window to focus when the icon for `app_id` is clicked.
    fn window_for(&self, app_id: &str) -> Option<ExtForeignToplevelHandleV1> {
        let is_open = |handle: &ExtForeignToplevelHandleV1| {
            self.toplevels.iter().any(|(open, _)| open == handle)
        };

        self.last_focused
            .get(app_id)
            .filter(|handle| is_open(*handle))
            .cloned()
            .or_else(|| {
                self.toplevels
                    .iter()
                    .find(|(_, id)| id == app_id)
                    .map(|(handle, _)| handle.clone())
            })
    }

    fn launch(&self, entry: Arc<fde::DesktopEntry>) {
        let Some(conn) = self.zbus.clone() else {
            tracing::error!("no D-Bus session connection, cannot launch");
            return;
        };
        let locales: Arc<[String]> = Arc::from(self.locales.clone());

        tokio::spawn(async move {
            let launcher = cosmic_launch::EntryLauncher::new(entry, locales);
            if let Err(why) = cosmic_launch::spawn(
                &conn,
                &launcher,
                None,
                &[],
                None,
                Vec::<(String, String)>::new(),
                &[] as &[String],
            )
            .await
            {
                tracing::error!(?why, "failed to spawn application");
            }
        });
    }

    /// Drawn while there is nothing to show yet, so the applet is never an
    /// invisible 1x1 widget (which looks exactly like "the applet didn't load").
    fn placeholder(&self) -> Element<'_, Message> {
        self.core
            .applet
            .applet_tooltip::<Message>(
                self.core
                    .applet
                    .icon_button("document-open-recent-symbolic"),
                fl!("empty-tooltip"),
                false,
                Message::Surface,
                None,
            )
            .into()
    }

    fn icon_button<'a>(&'a self, item: &'a Item) -> Element<'a, Message> {
        let applet = &self.core.applet;
        let (icon_size, _) = applet.suggested_size(false);
        let (major_padding, cross_padding) = applet.suggested_padding(false);
        let (h_padding, v_padding) = if applet.is_horizontal() {
            (major_padding, cross_padding)
        } else {
            (cross_padding, major_padding)
        };

        let icon = cosmic::widget::icon(item.icon_source.as_cosmic_icon())
            // preferred icon size variant
            .size(128)
            .width(icon_size.into())
            .height(icon_size.into());

        let button = cosmic::widget::button::custom(
            cosmic::widget::layer_container(icon).center(Length::Fill),
        )
        .width(Length::Fixed((icon_size + 2 * h_padding) as f32))
        .height(Length::Fixed((icon_size + 2 * v_padding) as f32))
        .class(cosmic::theme::Button::AppletIcon)
        .on_press_down(Message::Open(item.app_id.clone()));

        applet
            .applet_tooltip::<Message>(button, item.name.clone(), false, Message::Surface, None)
            .into()
    }
}

impl cosmic::Application for RecentApps {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = APP_ID;

    fn init(core: cosmic::app::Core, _flags: ()) -> (Self, app::Task<Message>) {
        // Use the icon theme the desktop is set to, and keep following it when the
        // theme (or light/dark mode) changes.
        follow_desktop_icon_theme();

        let config_helper = Config::new(APP_ID, RecentAppsConfig::VERSION).ok();
        let config = config_helper
            .as_ref()
            .and_then(|helper| RecentAppsConfig::get_entry(helper).ok())
            .unwrap_or_default();

        let zbus = iced::futures::executor::block_on(zbus::Connection::session()).ok();

        let mut app = Self {
            core,
            locales: fde::get_languages_from_env(),
            config,
            config_helper,
            zbus,
            ..Default::default()
        };
        app.update_desktop_entries();
        app.rebuild_items();
        tracing::info!(
            desktop_entries = app.desktop_entries.len(),
            saved_recents = app.config.recent.len(),
            shown = app.items.len(),
            "recent apps applet started"
        );

        (app, Task::none())
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn system_theme_update(
        &mut self,
        _keys: &[&'static str],
        _new_theme: &cosmic::cosmic_theme::Theme,
    ) -> app::Task<Message> {
        follow_desktop_icon_theme();
        Task::none()
    }

    fn system_theme_mode_update(
        &mut self,
        _keys: &[&'static str],
        _new_mode: &ThemeMode,
    ) -> app::Task<Message> {
        follow_desktop_icon_theme();
        Task::none()
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        match message {
            Message::Wayland(update) => match update {
                WaylandUpdate::Init(tx) => {
                    self.tx = Some(tx);
                }
                WaylandUpdate::Finished => {
                    tracing::error!("Wayland subscription ended");
                    self.tx = None;
                }
                WaylandUpdate::Toplevel(ToplevelUpdate::Changed {
                    handle,
                    app_id,
                    activated,
                }) => {
                    match self.toplevels.iter_mut().find(|(h, _)| *h == handle) {
                        Some(slot) => slot.1 = app_id.clone(),
                        None => self.toplevels.push((handle.clone(), app_id.clone())),
                    }

                    if activated && !app_id.is_empty() {
                        self.last_focused.insert(app_id.clone(), handle);
                        self.record_use(&app_id);
                    }
                }
                WaylandUpdate::Toplevel(ToplevelUpdate::Removed(handle)) => {
                    self.toplevels.retain(|(h, _)| *h != handle);
                    self.last_focused.retain(|_, h| *h != handle);
                }
            },
            Message::ConfigUpdated(config) => {
                // Another panel's instance of this applet saved a new list.
                if self.config != config {
                    self.config = config;
                    self.rebuild_items();
                }
            }
            Message::IconThemeChanged => follow_desktop_icon_theme(),
            Message::Open(app_id) => {
                if let Some(handle) = self.window_for(&app_id) {
                    if let Some(tx) = self.tx.as_ref() {
                        let _ =
                            tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(handle)));
                    }
                } else if let Some(item) = self.items.iter().find(|i| i.app_id == app_id) {
                    self.launch(item.entry.clone());
                }
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Surface(a));
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            wayland_subscription().map(Message::Wayland),
            self.core
                .watch_config::<ToolkitIconTheme>(TOOLKIT_CONFIG_ID)
                .map(|_| Message::IconThemeChanged),
            self.core.watch_config(APP_ID).map(|u| {
                for why in u.errors {
                    tracing::error!(why = why.to_string(), "Error watching config");
                }
                Message::ConfigUpdated(u.config)
            }),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let content: Element<_> = if self.items.is_empty() {
            self.placeholder()
        } else {
            let buttons = self.items.iter().map(|item| self.icon_button(item));
            let spacing = self.core.applet.spacing as f32;

            if self.core.applet.is_horizontal() {
                Row::with_children(buttons)
                    .align_y(Alignment::Center)
                    .spacing(spacing)
                    .into()
            } else {
                Column::with_children(buttons)
                    .align_x(Alignment::Center)
                    .spacing(spacing)
                    .into()
            }
        };

        autosize(content, AUTOSIZE_MAIN_ID.clone())
            .limits(Limits::NONE.min_width(1.).min_height(1.))
            .into()
    }
}
