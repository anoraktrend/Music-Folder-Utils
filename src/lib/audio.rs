use std::path::Path;

/// Audio file format constants used across the application
/// These define all supported audio formats for file processing
/// Please update this list when adding new audio formats
/// ID3-based audio formats (MP3, AAC)
pub const ID3_EXTENSIONS: &[&str] = &["mp3", "aac"];

/// MP4-based audio formats (M4A, M4B, ALAC, etc.)
pub const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "m4p", "alac", "mp4"];

/// Vorbis-based audio formats (FLAC, OGG, Opus, etc.)
pub const VORBIS_EXTENSIONS: &[&str] = &["flac", "ogg", "oga", "opus", "spx"];

/// APE audio formats
pub const APE_EXTENSIONS: &[&str] = &["ape", "mpc", "wv"];

/// AIFF audio formats
pub const AIFF_EXTENSIONS: &[&str] = &["aiff", "aif"];

/// WAV audio formats
pub const WAV_EXTENSIONS: &[&str] = &["wav"];

/// Get all supported audio file extensions as a combined vector
pub fn get_all_audio_extensions() -> Vec<&'static str> {
    ID3_EXTENSIONS
        .iter()
        .chain(MP4_EXTENSIONS.iter())
        .chain(VORBIS_EXTENSIONS.iter())
        .chain(APE_EXTENSIONS.iter())
        .chain(AIFF_EXTENSIONS.iter())
        .chain(WAV_EXTENSIONS.iter())
        .copied()
        .collect()
}

/// Check if a file path has a supported audio extension
pub fn is_audio_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    get_all_audio_extensions()
        .iter()
        .any(|&supported_ext| supported_ext == ext)
}

/// Get audio extension categories for a given file extension
pub fn get_extension_category(ext: &str) -> Option<&'static str> {
    let ext_lower = ext.to_lowercase();

    if ID3_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("id3")
    } else if MP4_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("mp4")
    } else if VORBIS_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("vorbis")
    } else if APE_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("ape")
    } else if AIFF_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("aiff")
    } else if WAV_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("wav")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_audio_extensions() {
        let all_exts = get_all_audio_extensions();
        assert!(all_exts.contains(&"mp3"));
        assert!(all_exts.contains(&"flac"));
        assert!(all_exts.contains(&"m4a"));
        assert!(all_exts.contains(&"ogg"));
        assert!(all_exts.contains(&"wav"));
        assert!(all_exts.contains(&"aiff"));
        assert!(all_exts.contains(&"ape"));
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file("test.mp3"));
        assert!(is_audio_file("test.flac"));
        assert!(is_audio_file("test.MP3")); // Case insensitive
        assert!(is_audio_file("test.FLAC")); // Case insensitive
        assert!(!is_audio_file("test.txt"));
        assert!(!is_audio_file("test.jpg"));
        assert!(!is_audio_file("test"));
    }

    #[test]
    fn test_get_extension_category() {
        assert_eq!(get_extension_category("mp3"), Some("id3"));
        assert_eq!(get_extension_category("flac"), Some("vorbis"));
        assert_eq!(get_extension_category("m4a"), Some("mp4"));
        assert_eq!(get_extension_category("wav"), Some("wav"));
        assert_eq!(get_extension_category("txt"), None);
    }
}
