use crate::utils::get_timestamp;
use futures::Stream;
use futures::TryStreamExt;
use reqwest::Client;
use thiserror::Error;

use super::models::{ApiConfig, ConvertResponse, InitResponse};

const ORIGIN: &str = "https://v1.y2mate.nu";
const REFERER: &str = "https://v1.y2mate.nu/";

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("API returned error: {0}")]
    ApiError(String),

    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    #[error("Download URL not found")]
    NoDownloadUrl,
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Clone)]
pub struct ApiClient {
    config: ApiConfig,
}

impl ApiClient {
    pub fn new(config: ApiConfig) -> Self {
        Self { config }
    }

    pub fn with_user_id(user_id: String) -> Self {
        Self::new(ApiConfig {
            user_id,
            ..Default::default()
        })
    }

    /// Step 1: Initialize the conversion process
    /// Returns the convert URL with signature
    pub async fn init(&self) -> Result<String> {
        let timestamp = get_timestamp();
        let url = format!(
            "{}/init?u={}&t={}",
            self.config.base_init_url, self.config.user_id, timestamp
        );

        let client = Client::new();
        let response = client
            .get(&url)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| ApiError::ApiError(format!("Init request failed: {}", e)))?;

        let json: InitResponse = response
            .json()
            .await
            .map_err(|e| ApiError::InvalidResponse(format!("JSON decode error: {}", e)))?;

        if json.error != "0" {
            return Err(ApiError::ApiError(json.error));
        }

        Ok(json.convert_url)
    }

    /// Step 2 & 3: Convert and follow redirects if needed
    /// Returns the final response with download URL
    pub async fn convert(&self, convert_url: &str, video_id: &str) -> Result<ConvertResponse> {
        let timestamp = get_timestamp();
        let convert_url = format!("{}&v={}&f=mp3&t={}", convert_url, video_id, timestamp);

        let client = Client::new();
        // First call to convert endpoint
        let response = client
            .get(&convert_url)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| ApiError::ApiError(format!("Convert request failed: {}", e)))?;

        let json: ConvertResponse = response
            .json()
            .await
            .map_err(|e| ApiError::InvalidResponse(format!("JSON decode error: {}", e)))?;

        if json.error != 0 {
            return Err(ApiError::ApiError(format!("Error code: {}", json.error)));
        }

        // Check if redirect is needed
        if json.redirect == 1 && !json.redirect_url.is_empty() {
            // Follow redirect - call the redirect_url
            let timestamp = get_timestamp();
            let redirect_url = format!("{}&t={}", json.redirect_url, timestamp);

            let response = client
                .get(&redirect_url)
                .header("Origin", ORIGIN)
                .header("Referer", REFERER)
                .send()
                .await?
                .error_for_status()
                .map_err(|e| ApiError::ApiError(format!("Redirect request failed: {}", e)))?;

            let json: ConvertResponse = response
                .json()
                .await
                .map_err(|e| ApiError::InvalidResponse(format!("JSON decode error: {}", e)))?;

            if json.error != 0 {
                return Err(ApiError::ApiError(format!("Error code: {}", json.error)));
            }

            Ok(json)
        } else {
            Ok(json)
        }
    }

    /// Step 4: Download file with progress stream
    /// Returns (total_size, stream)
    pub async fn download_file_stream(
        &self,
        download_url: &str,
    ) -> Result<(Option<u64>, impl Stream<Item = Result<bytes::Bytes>>)> {
        let client = Client::new();
        let response = client
            .get(download_url)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| ApiError::ApiError(format!("Download request failed: {}", e)))?;

        let total_size = response.content_length();
        let stream = response
            .bytes_stream()
            .map_err(|e| ApiError::RequestError(e));

        Ok((total_size, stream))
    }

    /// Get download info (title, url) without downloading
    pub async fn get_download_info(&self, video_id: &str) -> Result<(String, String)> {
        // Step 1: Get convert URL
        let convert_url = self.init().await?;

        // Step 2 & 3: Convert and get download URL
        let convert_response = self.convert(&convert_url, video_id).await?;

        if convert_response.download_url.is_empty() {
            return Err(ApiError::NoDownloadUrl);
        }

        Ok((convert_response.title, convert_response.download_url))
    }
}
