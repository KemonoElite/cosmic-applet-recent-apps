// SPDX-License-Identifier: GPL-3.0-only

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> cosmic::iced::Result {
    // The panel starts applets with its own log filter (warn+). It re-logs an applet's stdout
    // at info level (dropped) but its stderr at error level (kept), so write to stderr or
    // nothing of ours reaches the journal. RUST_LOG still overrides the default filter.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("warn,cosmic_applet_recent_apps=info")
    });
    // Plain text, no timestamps: the journal adds its own and shows lines containing
    // colour codes as "[... blob data]".
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .init();
    let _ = tracing_log::LogTracer::init();

    tracing::info!("Starting recent apps applet with version {VERSION}");

    cosmic_applet_recent_apps::run()
}
