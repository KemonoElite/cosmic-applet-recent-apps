// SPDX-License-Identifier: GPL-3.0-only

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> cosmic::iced::Result {
    // The panel doesn't set RUST_LOG, so default to showing this applet's own
    // info logs (they end up in the user journal); RUST_LOG still overrides.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("warn,cosmic_applet_recent_apps=info")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let _ = tracing_log::LogTracer::init();

    tracing::info!("Starting recent apps applet with version {VERSION}");

    cosmic_applet_recent_apps::run()
}
