use crate::api::ApiClient;
use crate::ui::{DownloadMessage, DownloadView};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

type DownloadResult = Result<(String, Vec<u8>), String>;

pub struct DownloadApp {
    view: DownloadView,
    api_client: Arc<Mutex<ApiClient>>,
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
            api_client: Arc::new(Mutex::new(api_client)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    UiMessage(DownloadMessage),
}

pub fn update(app: &mut DownloadApp, message: Message) {
    match message {
        Message::UiMessage(ui_msg) => {
            app.view.update(ui_msg.clone());

            if let DownloadMessage::DownloadPressed = ui_msg {
                if !app.view.video_id.is_empty() && !app.view.is_downloading {
                    let video_id = app.view.video_id.clone();
                    let api_client = app.api_client.clone();

                    app.view.is_downloading = true;
                    app.view.status_message = format!("Downloading: {}", video_id);

                    // Spawn download in background
                    std::thread::spawn(move || {
                        let result = download_video_blocking(api_client, &video_id);
                        match result {
                            Ok((title, data)) => {
                                let filename = format!("{}.mp3",
                                    crate::utils::sanitize_filename(&title)
                                        .trim_matches(|c| c == '.' || c == ' ')
                                );

                                match save_file(&filename, &data) {
                                    Ok(path) => {
                                        println!("✓ Saved: {}", path.display());
                                    }
                                    Err(e) => {
                                        println!("✗ Failed to save: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("✗ Download failed: {}", e);
                            }
                        }
                    });
                }
            }
        }
    }
}

pub fn view(app: &DownloadApp) -> iced::Element<'_, Message> {
    app.view.view().map(Message::UiMessage)
}

/// Blocking download function to run in background thread
fn download_video_blocking(
    api_client: Arc<Mutex<ApiClient>>,
    video_id: &str,
) -> DownloadResult {
    // Create a new runtime for this thread
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return Err(format!("Failed to create runtime: {}", e)),
    };

    // Block on the async download
    rt.block_on(async {
        let client_guard = api_client.lock().unwrap();
        let client = &*client_guard;

        client
            .download_mp3(video_id)
            .await
            .map(|(title, bytes)| (title, bytes.to_vec()))
            .map_err(|e| e.to_string())
    })
}

fn save_file(filename: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    let mut path = std::path::PathBuf::from(".");
    path.push(filename);

    std::fs::write(&path, data)?;

    Ok(path)
}
