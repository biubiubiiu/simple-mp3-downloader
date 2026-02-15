use serde::{Deserialize, Serialize};

/// Response from the /init endpoint
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitResponse {
    #[serde(rename = "convertURL")]
    pub convert_url: String,
    pub error: String,
}

/// Response from the /convert endpoint
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConvertResponse {
    pub error: i32,
    #[serde(rename = "progressURL")]
    pub progress_url: String,
    #[serde(rename = "downloadURL")]
    pub download_url: String,
    #[serde(rename = "redirectURL")]
    pub redirect_url: String,
    #[serde(default)]
    pub redirect: i32,
    #[serde(default)]
    pub title: String,
}

/// Configuration for the API client
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub user_id: String,
    pub base_init_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            user_id: "uLYHx4FToXeloU3RJEEliN".to_string(),
            base_init_url: "https://eta.etacloud.org/api/v1".to_string(),
        }
    }
}
