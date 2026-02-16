#[derive(Debug, Clone)]
pub struct DownloadPlan {
    pub title: String,
    pub download_url: String,
    pub suggested_filename: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    Idle,
    Preparing,
    AwaitingSavePath,
    Downloading,
    Completed,
    Failed,
}
