use crate::api::ApiClient;
use crate::ui::{DownloadMessage, DownloadView};
use std::path::PathBuf;
use iced::Task;

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
    /// (Title, Download URL)
    DownloadInfoReceived(Result<(String, String), String>),
    /// (Selected Path, Download URL)
    FileSaveSelected(Option<PathBuf>, String),
    /// Final result after downloading and saving
    DownloadCompleted(Result<PathBuf, String>),
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
                            return Task::perform(
                                async move {
                                    // Run in dedicated thread/runtime to ensure reqwest context
                                    let result = std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async move {
                                            api_client.get_download_info(&video_id).await
                                        })
                                    }).join().unwrap();

                                    result.map_err(|e| e.to_string())
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
                    let sanitized_filename = format!("{}.mp3", 
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
                    let api_client = app.api_client.clone();
                    
                    // Step 3: Download file to selected path
                    return Task::perform(
                        async move {
                            let url = url.clone();
                            let path = path.clone();
                            
                             // Run in dedicated thread/runtime
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                rt.block_on(async move {
                                    api_client.download_file(&url).await
                                })
                            }).join().unwrap();

                            match result {
                                Ok(data) => {
                                    match std::fs::write(&path, data) {
                                        Ok(_) => Ok(path),
                                        Err(e) => Err(format!("File write error: {}", e)),
                                    }
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        Message::DownloadCompleted,
                    );
                }
                None => {
                    // User cancelled dialog
                    app.view.is_downloading = false;
                    app.view.status_message = "Download cancelled".to_string();
                }
            }
        }
        Message::DownloadCompleted(result) => {
            app.view.is_downloading = false;
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
