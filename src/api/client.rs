use crate::utils::get_timestamp;
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

    #[error("Invalid response format")]
    InvalidResponse,

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
        Self {
            config,
        }
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
            .await?;

        // Check HTTP status before parsing JSON
        if !response.status().is_success() {
            return Err(ApiError::ApiError(format!(
                "HTTP {}: Init request failed",
                response.status()
            )));
        }

        let json: InitResponse = response.json().await?;

        if json.error != "0" {
            return Err(ApiError::ApiError(json.error));
        }

        Ok(json.convert_url)
    }

    /// Step 2 & 3: Convert and follow redirects if needed
    /// Returns the final response with download URL
    pub async fn convert(&self, convert_url: &str, video_id: &str) -> Result<ConvertResponse> {
        let timestamp = get_timestamp();
        let convert_url = format!(
            "{}&v={}&f=mp3&t={}",
            convert_url, video_id, timestamp
        );

        let client = Client::new();
        // First call to convert endpoint
        let response = client
            .get(&convert_url)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .send()
            .await?;

        // Check HTTP status before parsing JSON
        if !response.status().is_success() {
            return Err(ApiError::ApiError(format!(
                "HTTP {}: Convert request failed",
                response.status()
            )));
        }

        let json: ConvertResponse = response.json().await?;

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
                .await?;

            // Check HTTP status before parsing JSON
            if !response.status().is_success() {
                return Err(ApiError::ApiError(format!(
                    "HTTP {}: Redirect request failed",
                    response.status()
                )));
            }

            let json: ConvertResponse = response.json().await?;

            if json.error != 0 {
                return Err(ApiError::ApiError(format!("Error code: {}", json.error)));
            }

            Ok(json)
        } else {
            Ok(json)
        }
    }

    /// Step 4: Download the MP3 file
    pub async fn download_file(&self, download_url: &str) -> Result<bytes::Bytes> {
        let client = Client::new();
        let response = client
            .get(download_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ApiError::ApiError(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }

        Ok(response.bytes().await?)
    }

    /// Complete workflow: init -> convert -> download
    pub async fn download_mp3(&self, video_id: &str) -> Result<(String, bytes::Bytes)> {
        // Step 1: Get convert URL
        let convert_url = self.init().await?;

        // Step 2 & 3: Convert and get download URL
        let convert_response = self.convert(&convert_url, video_id).await?;

        if convert_response.download_url.is_empty() {
            return Err(ApiError::NoDownloadUrl);
        }

        // Step 4: Download the file
        let file_data = self.download_file(&convert_response.download_url).await?;

        Ok((convert_response.title, file_data))
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

    #[tokio::test]
    async fn test_get_download_info() {
        let mut server = mockito::Server::new_async().await;

        // Mock init endpoint
        let init_response = crate::api::models::InitResponse {
            convert_url: format!("{}/convert?sig=test123", server.url()),
            error: "0".to_string(),
        };

        let mock_init = server
            .mock("GET", mockito::Matcher::Regex("/init.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&init_response).unwrap())
            .create_async()
            .await;

        // Mock convert endpoint
        let convert_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: format!("{}/download.mp3", server.url()),
            redirect_url: String::new(),
            redirect: 0,
            title: "Test Song".to_string(),
        };

        let mock_convert = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&convert_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });

        let result = client.get_download_info("test_video_id").await;

        assert!(result.is_ok());
        let (title, url) = result.unwrap();
        assert_eq!(title, "Test Song");
        assert_eq!(url, format!("{}/download.mp3", server.url()));

        mock_init.assert_async().await;
        mock_convert.assert_async().await;
    }

    #[tokio::test]
    async fn test_client_new() {
        let config = crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: "https://example.com".to_string(),
        };
        let _client = ApiClient::new(config);
        // Verify client was created successfully
    }

    #[tokio::test]
    async fn test_client_with_user_id() {
        let _client = ApiClient::with_user_id("test_user".to_string());
        // Verify client was created successfully
    }

    #[tokio::test]
    async fn test_init_success() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = crate::api::models::InitResponse {
            convert_url: "/convert?sig=test123".to_string(),
            error: "0".to_string(),
        };

        let mock = server
            .mock("GET", mockito::Matcher::Regex("/init.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&mock_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });

        let result = client.init().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/convert?sig=test123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_init_api_error() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = crate::api::models::InitResponse {
            convert_url: String::new(),
            error: "INVALID_USER".to_string(),
        };

        let mock = server
            .mock("GET", mockito::Matcher::Regex("/init.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&mock_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });

        let result = client.init().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::ApiError(_)));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_convert_success() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: "https://example.com/download.mp3".to_string(),
            redirect_url: String::new(),
            redirect: 0,
            title: "Test Song".to_string(),
        };

        let mock = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&mock_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });
        let convert_url = &format!("{}/convert?sig=test123", server.url());
        let result = client.convert(convert_url, "test_video_id").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.title, "Test Song");
        assert_eq!(response.download_url, "https://example.com/download.mp3");
        assert_eq!(response.error, 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_convert_with_redirect() {
        let mut server = mockito::Server::new_async().await;

        // First response requiring redirect
        let redirect_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: String::new(),
            redirect_url: format!("{}/redirect", server.url()),
            redirect: 1,
            title: String::new(),
        };

        // Final response after redirect
        let final_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: "https://example.com/download.mp3".to_string(),
            redirect_url: String::new(),
            redirect: 0,
            title: "Test Song".to_string(),
        };

        let mock1 = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&redirect_response).unwrap())
            .create_async()
            .await;

        let mock2 = server
            .mock("GET", mockito::Matcher::Regex(".*redirect.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&final_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });
        let convert_url = &format!("{}/convert?sig=test123", server.url());
        let result = client.convert(convert_url, "test_video_id").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.title, "Test Song");
        assert_eq!(response.download_url, "https://example.com/download.mp3");

        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_convert_error() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = crate::api::models::ConvertResponse {
            error: 400,
            progress_url: String::new(),
            download_url: String::new(),
            redirect_url: String::new(),
            redirect: 0,
            title: String::new(),
        };

        let mock = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&mock_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });
        let convert_url = &format!("{}/convert?sig=test123", server.url());
        let result = client.convert(convert_url, "test_video_id").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::ApiError(_)));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_file_success() {
        let mut server = mockito::Server::new_async().await;
        let test_data = b"Test MP3 data";

        let mock = server
            .mock("GET", mockito::Matcher::Regex(".*download.*".to_string()))
            .with_status(200)
            .with_header("content-type", "audio/mpeg")
            .with_body(test_data)
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });
        let download_url = &format!("{}/download/test.mp3", server.url());
        let result = client.download_file(download_url).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.as_ref(), test_data);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_file_error() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", mockito::Matcher::Regex(".*download.*".to_string()))
            .with_status(404)
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });
        let download_url = &format!("{}/download/test.mp3", server.url());
        let result = client.download_file(download_url).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::ApiError(_)));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_mp3_full_workflow() {
        let mut server = mockito::Server::new_async().await;

        // Mock init endpoint
        let init_response = crate::api::models::InitResponse {
            convert_url: format!("{}/convert?sig=test123", server.url()),
            error: "0".to_string(),
        };

        let mock_init = server
            .mock("GET", mockito::Matcher::Regex("/init.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&init_response).unwrap())
            .create_async()
            .await;

        // Mock convert endpoint
        let convert_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: format!("{}/download.mp3", server.url()),
            redirect_url: String::new(),
            redirect: 0,
            title: "Test Song".to_string(),
        };

        let mock_convert = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&convert_response).unwrap())
            .create_async()
            .await;

        // Mock download endpoint
        let test_data = b"Test MP3 file content";
        let mock_download = server
            .mock("GET", "/download.mp3")
            .with_status(200)
            .with_header("content-type", "audio/mpeg")
            .with_body(test_data)
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });

        let result = client.download_mp3("test_video_id").await;

        assert!(result.is_ok());
        let (title, data) = result.unwrap();
        assert_eq!(title, "Test Song");
        assert_eq!(data.as_ref(), test_data);

        mock_init.assert_async().await;
        mock_convert.assert_async().await;
        mock_download.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_mp3_no_download_url() {
        let mut server = mockito::Server::new_async().await;

        // Mock init endpoint
        let init_response = crate::api::models::InitResponse {
            convert_url: format!("{}/convert?sig=test123", server.url()),
            error: "0".to_string(),
        };

        let mock_init = server
            .mock("GET", mockito::Matcher::Regex("/init.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&init_response).unwrap())
            .create_async()
            .await;

        // Mock convert endpoint with empty download URL
        let convert_response = crate::api::models::ConvertResponse {
            error: 0,
            progress_url: String::new(),
            download_url: String::new(), // Empty download URL
            redirect_url: String::new(),
            redirect: 0,
            title: "Test Song".to_string(),
        };

        let mock_convert = server
            .mock("GET", mockito::Matcher::Regex(".*convert.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&serde_json::to_string(&convert_response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(crate::api::models::ApiConfig {
            user_id: "test_user".to_string(),
            base_init_url: server.url(),
        });

        let result = client.download_mp3("test_video_id").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NoDownloadUrl));

        mock_init.assert_async().await;
        mock_convert.assert_async().await;
    }

    #[test]
    fn test_api_error_display() {
        let error = ApiError::ApiError("test error".to_string());
        assert_eq!(format!("{error}"), "API returned error: test error");

        let error = ApiError::InvalidResponse;
        assert_eq!(format!("{error}"), "Invalid response format");

        let error = ApiError::NoDownloadUrl;
        assert_eq!(format!("{error}"), "Download URL not found");
    }
}
