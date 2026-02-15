use iced::{
    widget::{button, column, text, text_input, Space},
    Element, Length,
};

/// Main view state
pub struct DownloadView {
    pub youtube_url: String,
    pub status_message: String,
    pub is_downloading: bool,
}

impl Default for DownloadView {
    fn default() -> Self {
        Self {
            youtube_url: String::new(),
            status_message: "Enter a video ID to download".to_string(),
            is_downloading: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    YoutubeUrlChanged(String),
    DownloadPressed,
}

impl DownloadView {
    pub fn update(&mut self, message: DownloadMessage) {
        match message {
            DownloadMessage::YoutubeUrlChanged(id) => {
                self.youtube_url = id;
            }
            DownloadMessage::DownloadPressed => {
                // Will be handled by the app
            }
        }
    }

    pub fn view(&self) -> Element<'_, DownloadMessage> {
        column![
            text("MP3 Downloader").size(32),
            Space::new().height(Length::Fixed(20.0)),
            text("YouTube URL:").size(16),
            text_input("Enter YouTube URL...", &self.youtube_url)
                .on_input(DownloadMessage::YoutubeUrlChanged)
                .padding(10),
            Space::new().height(Length::Fixed(10.0)),
            text(&self.status_message).size(14),
            Space::new().height(Length::Fixed(20.0)),
            button("Download MP3")
                .on_press(DownloadMessage::DownloadPressed)
                .padding([10, 20]),
        ]
        .padding(20)
        .spacing(10)
        .into()
    }
}
