use crate::utils::get_timestamp;
use futures::Stream;
use futures::TryStreamExt;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
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

    #[error("Failed to extract auth data from page")]
    AuthExtractionError,
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

    fn extract_json_from_html(&self, html: &str) -> Option<Value> {
        // Matches JSON.parse('...') inside the script tag
        let re = Regex::new(r"JSON\.parse\('([^']+)'\)").ok()?;
        if let Some(caps) = re.captures(html) {
            let json_str = &caps[1];
            return serde_json::from_str(json_str).ok();
        }
        None
    }

    fn calculate_authorization(&self, json: &Value) -> Option<(String, String)> {
        let j0 = json[0].as_array()?;
        let j1 = json[1].as_i64().unwrap_or(0);
        let j2 = json[2].as_array()?;

        let mut e = String::new();
        let j2_len = j2.len();

        for t in 0..j0.len() {
            let val0 = j0[t].as_i64()?;
            let val2 = j2[j2_len - (t + 1)].as_i64()?;
            let char_code = (val0 - val2) as u8;
            e.push(char_code as char);
        }

        if j1 != 0 {
            e = e.chars().rev().collect();
        }

        if e.len() > 32 {
            e.truncate(32);
        }

        // Get param name from json[6]
        let param_name = json[6]
            .as_u64()
            .map(|n| (n as u8) as char)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "u".to_string());

        Some((param_name, e))
    }

    /// Step 1: Initialize the conversion process
    /// Returns the convert URL with signature
    pub async fn init(&self) -> Result<String> {
        let client = Client::new();

        // 1. Fetch the main page to get the auth JSON
        let html = client.get(ORIGIN).send().await?.text().await?;

        // 2. Extract and calculate auth
        let json_val = self
            .extract_json_from_html(&html)
            .ok_or(ApiError::AuthExtractionError)?;
        let (param_name, auth_token) = self
            .calculate_authorization(&json_val)
            .ok_or(ApiError::AuthExtractionError)?;

        let timestamp = get_timestamp();
        let url = format!(
            "{}/init?{}={}&t={}",
            self.config.base_init_url, param_name, auth_token, timestamp
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_json() {
        let client = ApiClient::new(ApiConfig::default());
        let html = r#"var json = JSON.parse('[[94,118,116,80,77,82,93,66,85,115,110,104,93,123,96,70,57,131,82,95,78,131],1,[14,2,6,10,11,5,0,12,12,5,3,2,4,0,15,11,8,8,11,8,13,16],1,9,3,117]');"#;
        let json = client.extract_json_from_html(html).unwrap();
        assert!(json.is_array());
        assert_eq!(json[6].as_u64().unwrap(), 117);
    }

    #[test]
    fn test_calculate_authorization() {
        let client = ApiClient::new(ApiConfig::default());
        let json_val = json!([
            [
                94, 118, 116, 80, 77, 82, 93, 66, 85, 115, 110, 104, 93, 123, 96, 70, 57, 131, 82,
                95, 78, 131
            ],
            1,
            [14, 2, 6, 10, 11, 5, 0, 12, 12, 5, 3, 2, 4, 0, 15, 11, 8, 8, 11, 8, 13, 16],
            1,
            9,
            3,
            117
        ]);
        let (param, auth) = client.calculate_authorization(&json_val).unwrap();
        assert_eq!(param, "u");
        // Manual calculation check:
        // t=0: 94-16 = 78 (N)
        // t=1: 118-13 = 105 (i)
        // t=2: 116-8 = 108 (l)
        // ...
        // and reverse because json[1] is 1
        assert_eq!(auth.len(), 22);
        assert!(auth.ends_with("N")); // Reversed, so N is at the end
        assert_eq!(auth, "uLYHx4FToXeloU3RJEEliN")
    }
}
