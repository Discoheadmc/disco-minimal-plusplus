//! Disco Minimal++ — Freya uygulaması.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![recursion_limit = "256"]

mod actions;
mod components;
mod state;
mod theme;
mod view;

#[cfg(test)]
mod ui_tests;

use freya::prelude::*;
use freya::radio::use_init_radio_station;

use crate::state::{AppChannel, AppState};

pub struct DiscoApp;

impl App for DiscoApp {
    fn render(&self) -> impl IntoElement {
        let station = use_init_radio_station::<AppState, AppChannel>(AppState::initial);

        let actions = use_hook(move || {
            let actions = actions::Actions::new(station);
            actions.refresh_instances();
            actions.refresh_manifest();
            actions
        });

        actions::use_provide_actions(&actions);

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(theme::bg())
            .child(view::Home)
    }
}

fn main() {
    use tracing_subscriber::EnvFilter;

    // Varsayılan: bizim crate'ler info, freya iç katmanları sessiz.
    // DISCO_LOG ortam değişkeniyle özelleştirilebilir.
    let default_filter = "warn,disco_app=info,disco_core=info,disco_mc=info,disco_net=info";
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("DISCO_LOG").unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .init();

    // Freya sdk-tokio, akışların aynı runtime'ta koşması için runtime'ın
    // kurulu ve girilmiş olmasını bekler.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime oluşturulamadı");
    let _tokio_guard = rt.enter();

    if let Err(err) = disco_common::paths::ensure_layout() {
        tracing::error!("veri dizinleri oluşturulamadı: {err}");
    }

    let window = WindowConfig::new_app(DiscoApp)
        .with_title(disco_common::constants::WINDOW_TITLE)
        .with_app_id("disco_minimal_plusplus")
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../icons/128x128.png"
        )))
        .with_size(1180., 760.)
        .with_min_size(920., 640.)
        .with_decorations(false);

    let launch_config = LaunchConfig::new()
        .with_window(window)
        .with_plugin(freya::borderless::BorderlessPlugin::new());

    launch(launch_config);
}
