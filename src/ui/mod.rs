use iced::{
    widget::{button, column, text, text_input, Space},
    Element, Length,
};

/// Main view state
pub struct DownloadView {
    pub video_id: String,
    pub status_message: String,
    pub is_downloading: bool,
}

impl Default for DownloadView {
    fn default() -> Self {
        Self {
            video_id: String::new(),
            status_message: "Enter a video ID to download".to_string(),
            is_downloading: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    VideoIdChanged(String),
    DownloadPressed,
}

impl DownloadView {
    pub fn update(&mut self, message: DownloadMessage) {
        match message {
            DownloadMessage::VideoIdChanged(id) => {
                self.video_id = id;
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
            text("Video ID:").size(16),
            text_input("Enter video ID...", &self.video_id)
                .on_input(DownloadMessage::VideoIdChanged)
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
