use std::path::PathBuf;

use futures::{stream::BoxStream, StreamExt};
use tokio::io::AsyncWriteExt;

use crate::{
    api::ApiClient,
    domain::{AppError, DownloadPlan},
    utils::{extract_video_id, sanitize_filename},
};

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress(f32),
    Completed(PathBuf),
    Failed(AppError),
}

#[derive(Clone)]
pub struct DownloadCoordinator {
    api_client: ApiClient,
}

impl DownloadCoordinator {
    pub fn new(api_client: ApiClient) -> Self {
        Self { api_client }
    }

    pub async fn prepare_download(&self, youtube_url: String) -> Result<DownloadPlan, AppError> {
        let video_id = extract_video_id(&youtube_url).ok_or(AppError::InvalidInput)?;

        let (title, download_url) = self
            .api_client
            .get_download_info(&video_id)
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        let suggested_filename = format!(
            "{}.mp3",
            sanitize_filename(&title).trim_matches(|c| c == '.' || c == ' ')
        );

        Ok(DownloadPlan {
            title,
            download_url,
            suggested_filename,
        })
    }

    pub async fn choose_save_path(&self, suggested_filename: String) -> Option<PathBuf> {
        rfd::AsyncFileDialog::new()
            .set_file_name(&suggested_filename)
            .save_file()
            .await
            .map(|handle| handle.path().to_path_buf())
    }

    pub fn download_stream(&self, url: String, path: PathBuf) -> BoxStream<'static, DownloadEvent> {
        futures::stream::unfold(
            DownloadRuntimeState::Start {
                client: self.api_client.clone(),
                url,
                path,
            },
            |state| async move {
                match state {
                    DownloadRuntimeState::Start { client, url, path } => {
                        let file = match tokio::fs::File::create(&path).await {
                            Ok(file) => file,
                            Err(e) => {
                                return Some((
                                    DownloadEvent::Failed(AppError::Io(format!(
                                        "Failed to create file: {}",
                                        e
                                    ))),
                                    DownloadRuntimeState::Finished,
                                ));
                            }
                        };

                        match client.download_file_stream(&url).await {
                            Ok((total_size, stream)) => Some((
                                DownloadEvent::Progress(0.0),
                                DownloadRuntimeState::Downloading {
                                    file,
                                    stream: stream.boxed(),
                                    downloaded: 0,
                                    total: total_size,
                                    path,
                                },
                            )),
                            Err(e) => Some((
                                DownloadEvent::Failed(AppError::Api(e.to_string())),
                                DownloadRuntimeState::Finished,
                            )),
                        }
                    }
                    DownloadRuntimeState::Downloading {
                        mut file,
                        mut stream,
                        mut downloaded,
                        total,
                        path,
                    } => match stream.next().await {
                        Some(Ok(chunk)) => {
                            if let Err(e) = file.write_all(&chunk).await {
                                return Some((
                                    DownloadEvent::Failed(AppError::Io(format!(
                                        "Write error: {}",
                                        e
                                    ))),
                                    DownloadRuntimeState::Finished,
                                ));
                            }

                            downloaded += chunk.len() as u64;

                            let progress = if let Some(total_size) = total {
                                if total_size > 0 {
                                    downloaded as f32 / total_size as f32
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            };

                            Some((
                                DownloadEvent::Progress(progress),
                                DownloadRuntimeState::Downloading {
                                    file,
                                    stream,
                                    downloaded,
                                    total,
                                    path,
                                },
                            ))
                        }
                        Some(Err(e)) => Some((
                            DownloadEvent::Failed(AppError::Api(e.to_string())),
                            DownloadRuntimeState::Finished,
                        )),
                        None => {
                            if let Err(e) = file.sync_all().await {
                                return Some((
                                    DownloadEvent::Failed(AppError::Io(format!(
                                        "Failed to sync file: {}",
                                        e
                                    ))),
                                    DownloadRuntimeState::Finished,
                                ));
                            }

                            Some((
                                DownloadEvent::Completed(path),
                                DownloadRuntimeState::Finished,
                            ))
                        }
                    },
                    DownloadRuntimeState::Finished => None,
                }
            },
        )
        .boxed()
    }
}

enum DownloadRuntimeState {
    Start {
        client: ApiClient,
        url: String,
        path: PathBuf,
    },
    Downloading {
        file: tokio::fs::File,
        stream: BoxStream<'static, crate::api::Result<bytes::Bytes>>,
        downloaded: u64,
        total: Option<u64>,
        path: PathBuf,
    },
    Finished,
}
