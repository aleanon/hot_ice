use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use ui::State;

#[cfg(feature = "reload")]
use hot_ice::application;
#[cfg(not(feature = "reload"))]
use hot_ice::iced::application;

fn main() {
    init_tracing();

    #[cfg(feature = "reload")]
    let reloader_settings = hot_ice::ReloaderSettings {
        compile_in_reloader: false,
        ..Default::default()
    };

    let app = application(State::boot, State::update, State::view)
        .subscription(State::subscription)
        .theme(State::theme)
        .style(State::style)
        .scale_factor(State::scale_factor)
        .title(State::title);

    #[cfg(not(feature = "reload"))]
    app.run().unwrap();

    #[cfg(feature = "reload")]
    app.reloader_settings(reloader_settings).run().unwrap();
}

pub fn init_tracing() {
    let fmt_layer = fmt::layer().compact();

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}
