// SPDX-License-Identifier: GPL-3.0-only

use cosmic_config::{self, Config, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "com.system76.CosmicAppletRecentApps";

/// How many recent apps are shown in the panel.
pub const MAX_SHOWN: usize = 3;

/// How many app ids are remembered. This is larger than `MAX_SHOWN` so that
/// an app that has since been uninstalled doesn't leave the applet short.
pub const HISTORY_LEN: usize = 12;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, CosmicConfigEntry)]
#[version = 1]
pub struct RecentAppsConfig {
    /// Window `app_id`s, most recently used first.
    pub recent: Vec<String>,
}

impl RecentAppsConfig {
    /// Moves `app_id` to the front of the list.
    /// Returns `false` (and changes nothing) if it was already first.
    pub fn touch(&mut self, app_id: &str) -> bool {
        if self.recent.first().is_some_and(|first| first == app_id) {
            return false;
        }
        self.recent.retain(|id| id != app_id);
        self.recent.insert(0, app_id.to_owned());
        self.recent.truncate(HISTORY_LEN);
        true
    }

    pub fn save(&self, helper: &Config) {
        if let Err(why) = self.write_entry(helper) {
            tracing::error!(?why, "failed to save recent apps");
        }
    }
}
