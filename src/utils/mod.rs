use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds
pub fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Sanitize filename to remove invalid characters
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Extract video ID from various YouTube URL formats
/// Supports:
/// - https://www.youtube.com/watch?v=VIDEO_ID
/// - https://youtu.be/VIDEO_ID
/// - https://youtube.com/watch?v=VIDEO_ID
/// - Direct video ID (returns as-is if valid)
pub fn extract_video_id(input: &str) -> Option<String> {
    let input = input.trim();

    // If it looks like a raw video ID (11 characters, typical YouTube ID format)
    if input.len() == 11
        && input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some(input.to_string());
    }

    // Try to parse as URL
    if let Ok(url) = url::Url::parse(input) {
        // Handle youtu.be short URLs
        if url.host_str().map_or(false, |h| h.ends_with("youtu.be")) {
            return url.path_segments()?.last().map(String::from);
        }

        // Handle youtube.com watch URLs
        if url.host_str().map_or(false, |h| h.ends_with("youtube.com")) {
            return url
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = get_timestamp();
        assert!(ts > 1700000000); // Sanity check
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test/file.mp3"), "test_file.mp3");
        assert_eq!(sanitize_filename("normal-name.mp3"), "normal-name.mp3");
    }

    #[test]
    fn test_extract_video_id_from_watch_url() {
        let url = "https://www.youtube.com/watch?v=z0vCwGUZe1I";
        assert_eq!(extract_video_id(url), Some("z0vCwGUZe1I".to_string()));
    }

    #[test]
    fn test_extract_video_id_from_short_url() {
        let url = "https://youtu.be/z0vCwGUZe1I";
        assert_eq!(extract_video_id(url), Some("z0vCwGUZe1I".to_string()));
    }

    #[test]
    fn test_extract_video_id_raw() {
        let video_id = "z0vCwGUZe1I";
        assert_eq!(extract_video_id(video_id), Some("z0vCwGUZe1I".to_string()));
    }

    #[test]
    fn test_extract_video_id_invalid() {
        assert_eq!(extract_video_id("not a url"), None);
        assert_eq!(extract_video_id("https://example.com"), None);
    }
}
