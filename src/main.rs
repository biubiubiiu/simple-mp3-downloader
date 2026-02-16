mod api;
mod app;
mod application;
mod domain;
mod ui;
mod utils;

use iced::window;

fn main() -> iced::Result {
    let icon_data = include_bytes!("../assets/icon.png");

    let icon = match image::load_from_memory(icon_data) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            window::icon::from_rgba(rgba.into_raw(), width, height).ok()
        }
        Err(_) => None,
    };

    iced::application(app::DownloadApp::default, app::update, app::view)
        .title("Simple MP3 Downloader")
        .window(window::Settings {
            icon,
            ..Default::default()
        })
        .run()
}
