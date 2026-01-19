use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use ui::State;

fn main() {
    init_tracing();

    hot_ice::application(State::boot, State::update, State::view)
        .subscription(State::subscription)
        .theme(State::theme)
        .style(State::style)
        .scale_factor(State::scale_factor)
        .title(State::title)
        .run()
        .unwrap();
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
