use iced::{
    widget::{button, column, progress_bar, text, text_input, Space},
    Element, Length,
};

/// Main view state
pub struct DownloadView {
    pub youtube_url: String,
    pub status_message: String,
    pub is_downloading: bool,
    pub download_progress: f32,
}

impl Default for DownloadView {
    fn default() -> Self {
        Self {
            youtube_url: String::new(),
            status_message: "Enter a youtube video url to download".to_string(),
            is_downloading: false,
            download_progress: 0.0,
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
        let progress_bar = if self.is_downloading {
            Some(progress_bar(0.0..=1.0, self.download_progress))
        } else {
            None
        };

        let mut content = column![
            text("MP3 Downloader").size(32),
            Space::new().height(Length::Fixed(20.0)),
            text("YouTube URL:").size(16),
            text_input("Enter YouTube URL...", &self.youtube_url)
                .on_input(DownloadMessage::YoutubeUrlChanged)
                .padding(10),
            Space::new().height(Length::Fixed(10.0)),
            text(&self.status_message).size(14),
        ];

        // Add progress bar if downloading
        if let Some(pb) = progress_bar {
            content = content
                .push(Space::new().height(Length::Fixed(10.0)))
                .push(pb);
        }

        content = content.push(Space::new().height(Length::Fixed(20.0))).push(
            button("Download MP3")
                .on_press_maybe(if !self.is_downloading {
                    Some(DownloadMessage::DownloadPressed)
                } else {
                    None
                })
                .padding([10, 20]),
        );

        content.padding(20).spacing(10).into()
    }
}
