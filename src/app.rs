use std::path::PathBuf;

use futures::StreamExt;
use iced::Task;

use crate::{
    api::ApiClient,
    application::{DownloadCoordinator, DownloadEvent},
    domain::{AppError, DownloadPhase, DownloadPlan},
    ui::{DownloadMessage, DownloadView},
};

pub struct DownloadApp {
    view: DownloadView,
    coordinator: DownloadCoordinator,
    phase: DownloadPhase,
    active_plan: Option<DownloadPlan>,
}

impl Default for DownloadApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadApp {
    pub fn new() -> Self {
        let api_client = ApiClient::new(Default::default());

        Self {
            view: DownloadView::default(),
            coordinator: DownloadCoordinator::new(api_client),
            phase: DownloadPhase::Idle,
            active_plan: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Ui(DownloadMessage),
    Prepared(Result<DownloadPlan, AppError>),
    SavePathChosen(Option<PathBuf>),
    Download(DownloadEvent),
}

pub fn update(app: &mut DownloadApp, message: Message) -> Task<Message> {
    match message {
        Message::Ui(ui_msg) => {
            app.view.update(ui_msg.clone());

            if let DownloadMessage::DownloadPressed = ui_msg {
                if app.phase == DownloadPhase::Downloading {
                    return Task::none();
                }

                app.phase = DownloadPhase::Preparing;
                app.view.is_downloading = true;
                app.view.download_progress = 0.0;
                app.view.status_message = "Fetching download info...".to_string();

                let coordinator = app.coordinator.clone();
                let youtube_url = app.view.youtube_url.clone();

                return Task::perform(
                    async move { coordinator.prepare_download(youtube_url).await },
                    Message::Prepared,
                );
            }
        }
        Message::Prepared(result) => match result {
            Ok(plan) => {
                app.phase = DownloadPhase::AwaitingSavePath;
                app.view.status_message =
                    format!("Ready: {}. Please select save location...", plan.title);
                app.active_plan = Some(plan.clone());

                let coordinator = app.coordinator.clone();
                let suggested_filename = plan.suggested_filename;

                return Task::perform(
                    async move { coordinator.choose_save_path(suggested_filename).await },
                    Message::SavePathChosen,
                );
            }
            Err(e) => {
                app.phase = DownloadPhase::Failed;
                app.view.is_downloading = false;
                app.view.download_progress = 0.0;
                app.view.status_message = format_error("Failed to prepare download", &e);
            }
        },
        Message::SavePathChosen(path_opt) => match path_opt {
            Some(path) => {
                if let Some(plan) = app.active_plan.take() {
                    app.phase = DownloadPhase::Downloading;
                    app.view.is_downloading = true;
                    app.view.download_progress = 0.0;
                    app.view.status_message = format!("Downloading to: {}", path.display());

                    return Task::stream(
                        app.coordinator
                            .download_stream(plan.download_url, path)
                            .map(Message::Download),
                    );
                }

                app.phase = DownloadPhase::Failed;
                app.view.is_downloading = false;
                app.view.status_message = "Missing download plan".to_string();
            }
            None => {
                app.phase = DownloadPhase::Idle;
                app.active_plan = None;
                app.view.is_downloading = false;
                app.view.download_progress = 0.0;
                app.view.status_message = "Download cancelled".to_string();
            }
        },
        Message::Download(event) => match event {
            DownloadEvent::Progress(progress) => {
                app.phase = DownloadPhase::Downloading;
                app.view.download_progress = progress;

                if progress >= 1.0 {
                    app.view.status_message = "Download complete, finalizing...".to_string();
                } else {
                    app.view.status_message = format!("Downloading: {:.1}%", progress * 100.0);
                }
            }
            DownloadEvent::Completed(path) => {
                app.phase = DownloadPhase::Completed;
                app.view.is_downloading = false;
                app.view.download_progress = 0.0;
                app.view.status_message = format!("Saved: {}", path.display());
            }
            DownloadEvent::Failed(error) => {
                app.phase = DownloadPhase::Failed;
                app.view.is_downloading = false;
                app.view.download_progress = 0.0;
                app.view.status_message = format_error("Download failed", &error);
            }
        },
    }

    Task::none()
}

pub fn view(app: &DownloadApp) -> iced::Element<'_, Message> {
    app.view.view().map(Message::Ui)
}

fn format_error(prefix: &str, error: &AppError) -> String {
    format!("{}: {}", prefix, error)
}
