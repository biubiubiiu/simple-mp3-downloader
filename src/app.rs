use crate::api::ApiClient;
use crate::ui::{DownloadMessage, DownloadView};
use futures::StreamExt;
use iced::Task;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub struct DownloadApp {
    view: DownloadView,
    api_client: ApiClient,
    // Store download state for subscription
    pending_download: Option<(String, PathBuf)>, // (url, save_path)
}

impl Default for DownloadApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadApp {
    pub fn new() -> Self {
        let api_client = ApiClient::new(Default::default());
        let view = DownloadView::default();

        Self {
            view,
            api_client,
            pending_download: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    UiMessage(DownloadMessage),
    /// (Title, Download URL)
    DownloadInfoReceived(Result<(String, String), String>),
    /// (Selected Path, Download URL)
    FileSaveSelected(Option<PathBuf>, String),
    /// Download progress (0.0 to 1.0)
    DownloadProgress(f32),
    /// Final result after downloading and saving
    DownloadCompleted(Result<PathBuf, String>),
}

/// Internal state for the download stream
enum DownloadState {
    Start {
        client: ApiClient,
        url: String,
        path: PathBuf,
    },
    Downloading {
        file: tokio::fs::File,
        stream: futures::stream::BoxStream<'static, crate::api::Result<bytes::Bytes>>,
        downloaded: u64,
        total: Option<u64>,
        path: PathBuf,
    },
    Finished,
}

pub fn update(app: &mut DownloadApp, message: Message) -> Task<Message> {
    match message {
        Message::UiMessage(ui_msg) => {
            app.view.update(ui_msg.clone());

            if let DownloadMessage::DownloadPressed = ui_msg {
                if !app.view.youtube_url.is_empty() && !app.view.is_downloading {
                    // Extract video ID from URL
                    match crate::utils::extract_video_id(&app.view.youtube_url) {
                        Some(video_id) => {
                            let api_client = app.api_client.clone();

                            app.view.is_downloading = true;
                            app.view.status_message = format!("Fetching info for: {}", video_id);

                            // Step 1: Get download URL and title
                            // iced Task::perform runs in the background tokio executor
                            return Task::perform(
                                async move {
                                    api_client
                                        .get_download_info(&video_id)
                                        .await
                                        .map_err(|e| e.to_string())
                                },
                                Message::DownloadInfoReceived,
                            );
                        }
                        None => {
                            app.view.status_message = "Invalid YouTube URL or video ID".to_string();
                        }
                    }
                }
            }
        }
        Message::DownloadInfoReceived(result) => {
            match result {
                Ok((title, url)) => {
                    app.view.status_message = "Please select save location...".to_string();
                    let sanitized_filename = format!(
                        "{}.mp3",
                        crate::utils::sanitize_filename(&title)
                            .trim_matches(|c| c == '.' || c == ' ')
                    );

                    // Step 2: Open Save Dialog
                    return Task::perform(
                        async move {
                            let path = rfd::AsyncFileDialog::new()
                                .set_file_name(&sanitized_filename)
                                .save_file()
                                .await
                                .map(|handle| handle.path().to_path_buf());

                            (path, url)
                        },
                        |(path, url)| Message::FileSaveSelected(path, url),
                    );
                }
                Err(e) => {
                    app.view.is_downloading = false;
                    app.view.status_message = format!("Failed to get info: {}", e);
                }
            }
        }
        Message::FileSaveSelected(path_opt, url) => {
            match path_opt {
                Some(path) => {
                    app.view.status_message = format!("Downloading to: {}", path.display());
                    app.pending_download = Some((url.clone(), path.clone()));

                    let api_client = app.api_client.clone();

                    // Step 3: Start streaming download
                    return Task::stream(futures::stream::unfold(
                        DownloadState::Start {
                            client: api_client,
                            url,
                            path,
                        },
                        |state| async move {
                            match state {
                                DownloadState::Start { client, url, path } => {
                                    // Create file asynchronously
                                    let file = match tokio::fs::File::create(&path).await {
                                        Ok(f) => f,
                                        Err(e) => {
                                            return Some((
                                                Message::DownloadCompleted(Err(format!(
                                                    "Failed to create file: {}",
                                                    e
                                                ))),
                                                DownloadState::Finished,
                                            ))
                                        }
                                    };

                                    // Request download stream
                                    match client.download_file_stream(&url).await {
                                        Ok((total_size, stream)) => Some((
                                            Message::DownloadProgress(0.0),
                                            DownloadState::Downloading {
                                                file,
                                                stream: stream.boxed(),
                                                downloaded: 0,
                                                total: total_size,
                                                path,
                                            },
                                        )),
                                        Err(e) => Some((
                                            Message::DownloadCompleted(Err(e.to_string())),
                                            DownloadState::Finished,
                                        )),
                                    }
                                }
                                DownloadState::Downloading {
                                    mut file,
                                    mut stream,
                                    mut downloaded,
                                    total,
                                    path,
                                } => {
                                    // Get next chunk from stream
                                    match stream.next().await {
                                        Some(Ok(chunk)) => {
                                            // Write chunk to file asynchronously
                                            if let Err(e) = file.write_all(&chunk).await {
                                                return Some((
                                                    Message::DownloadCompleted(Err(format!(
                                                        "Write error: {}",
                                                        e
                                                    ))),
                                                    DownloadState::Finished,
                                                ));
                                            }

                                            downloaded += chunk.len() as u64;

                                            // Calculate progress if total size is known
                                            let progress = if let Some(t) = total {
                                                if t > 0 {
                                                    downloaded as f32 / t as f32
                                                } else {
                                                    0.0
                                                }
                                            } else {
                                                0.0
                                            };

                                            Some((
                                                Message::DownloadProgress(progress),
                                                DownloadState::Downloading {
                                                    file,
                                                    stream,
                                                    downloaded,
                                                    total,
                                                    path,
                                                },
                                            ))
                                        }
                                        Some(Err(e)) => Some((
                                            Message::DownloadCompleted(Err(e.to_string())),
                                            DownloadState::Finished,
                                        )),
                                        None => {
                                            // Stream finished successfully
                                            // Flush remaining data to disk
                                            if let Err(e) = file.sync_all().await {
                                                return Some((
                                                    Message::DownloadCompleted(Err(format!(
                                                        "Failed to sync file: {}",
                                                        e
                                                    ))),
                                                    DownloadState::Finished,
                                                ));
                                            }

                                            Some((
                                                Message::DownloadCompleted(Ok(path)),
                                                DownloadState::Finished,
                                            ))
                                        }
                                    }
                                }
                                DownloadState::Finished => None,
                            }
                        },
                    ));
                }
                None => {
                    // User cancelled dialog
                    app.view.is_downloading = false;
                    app.view.status_message = "Download cancelled".to_string();
                }
            }
        }
        Message::DownloadProgress(progress) => {
            app.view.download_progress = progress;
            if progress >= 1.0 {
                app.view.status_message = "Download complete, finalizing...".to_string();
            } else {
                app.view.status_message = format!("Downloading: {:.1}%", progress * 100.0);
            }
        }
        Message::DownloadCompleted(result) => {
            app.view.is_downloading = false;
            app.pending_download = None;
            app.view.download_progress = 0.0;
            match result {
                Ok(path) => {
                    app.view.status_message = format!("Saved: {}", path.display());
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
