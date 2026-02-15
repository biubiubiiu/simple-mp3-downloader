use crate::api::ApiClient;
use crate::ui::{DownloadMessage, DownloadView};
use crate::utils::extract_video_id;
use std::path::PathBuf;
use iced::Task;

type DownloadResult = Result<(String, Vec<u8>), String>;

pub struct DownloadApp {
    view: DownloadView,
    api_client: ApiClient,
}

impl Default for DownloadApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadApp {
    pub fn new() -> Self {
        let api_client = ApiClient::with_user_id("uLYHx4FToXeloU3RJEEliN".to_string());
        let view = DownloadView::default();

        Self {
            view,
            api_client,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    UiMessage(DownloadMessage),
    DownloadCompleted(DownloadResult),
}

pub fn update(app: &mut DownloadApp, message: Message) -> Task<Message> {
    match message {
        Message::UiMessage(ui_msg) => {
            app.view.update(ui_msg.clone());

            if let DownloadMessage::DownloadPressed = ui_msg {
                if !app.view.video_id.is_empty() && !app.view.is_downloading {
                    let input = app.view.video_id.clone();

                    // Try to extract video ID from URL or use input directly
                    let video_id = match extract_video_id(&input) {
                        Some(id) => id,
                        None => {
                            app.view.status_message = "Invalid YouTube URL or video ID".to_string();
                            return Task::none();
                        }
                    };

                    let api_client = app.api_client.clone();

                    app.view.is_downloading = true;
                    app.view.status_message = format!("Downloading: {}", video_id);

                    return Task::perform(
                        async move {
                            let video_id = video_id.clone();
                            let api_client = api_client.clone();

                            // Create a new runtime for the blocking task to ensure reqwest has a reactor context
                            // This is necessary because Iced's default executor might not provide the tokio context reqwest expects
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                rt.block_on(async move {
                                    api_client
                                        .download_mp3(&video_id)
                                        .await
                                })
                            }).join().unwrap(); // Propagate panic if thread fails

                            result
                                .map(|(title, bytes)| (title, bytes.to_vec()))
                                .map_err(|e| e.to_string())
                        },
                        Message::DownloadCompleted,
                    );
                }
            }
        }
        Message::DownloadCompleted(result) => {
            app.view.is_downloading = false;
            match result {
                Ok((title, data)) => {
                    let filename = format!("{}.mp3",
                        crate::utils::sanitize_filename(&title)
                            .trim_matches(|c| c == '.' || c == ' ')
                    );

                    match save_file(&filename, &data) {
                        Ok(path) => {
                            app.view.status_message = format!("Saved: {}", path.display());
                        }
                        Err(e) => {
                            app.view.status_message = format!("Failed to save: {}", e);
                        }
                    }
                }
                Err(e) => {
                    app.view.status_message = format!("Download failed: {}", e);
                }
            }
        }
    }
    Task::none()
}

pub fn view(app: &DownloadApp) -> iced::Element<'_, Message> {
    app.view.view().map(Message::UiMessage)
}

fn save_file(filename: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    let mut path = std::path::PathBuf::from(".");
    path.push(filename);

    std::fs::write(&path, data)?;

    Ok(path)
}
