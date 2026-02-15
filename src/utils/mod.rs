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
}
