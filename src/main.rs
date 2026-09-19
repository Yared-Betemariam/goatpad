#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod appearance;
mod config;
mod domain;
mod editor;
mod input;
mod services;

use app::{GoatpadApp, resources};
use config::{APP_NAME, DEFAULT_WINDOW_SIZE, MIN_WINDOW_SIZE};
use eframe::egui;
use services::{paths::AppPaths, session::Session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let session = Session::load(&paths)?;
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(DEFAULT_WINDOW_SIZE)
        .with_min_inner_size(MIN_WINDOW_SIZE)
        .with_title(APP_NAME)
        .with_icon(resources::app_icon())
        .with_decorations(false);
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions {
            viewport,
            // Goatpad owns its session file, including native window geometry.
            // Keeping eframe's optional persistence disabled avoids two stores
            // competing to restore the same viewport.
            persist_window: false,
            ..Default::default()
        },
        Box::new(move |creation_context| {
            Ok(Box::new(GoatpadApp::new(
                paths,
                session,
                &creation_context.egui_ctx,
            )?))
        }),
    )?;
    Ok(())
}
