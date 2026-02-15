mod api;
mod app;
mod ui;
mod utils;

fn main() -> iced::Result {
    iced::run(app::update, app::view)
}
