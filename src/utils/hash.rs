//! Content hash utilities for generating unique identifiers.
//!
//! Provides deterministic hash functions for clipboard content deduplication.

use crate::repository::ContentType;

/// Compute a deterministic content hash using seahash.
///
/// The content type is encoded as a prefix byte to avoid collisions
/// between different types with the same content.
pub(crate) fn content_hash(content: &str, content_type: &ContentType) -> u64 {
    let type_tag = content_type.as_tag();
    let mut data = vec![type_tag];
    data.extend_from_slice(content.as_bytes());
    seahash::hash(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("test", &ContentType::Text);
        let h2 = content_hash("test", &ContentType::Text);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_different_content() {
        let h1 = content_hash("test1", &ContentType::Text);
        let h2 = content_hash("test2", &ContentType::Text);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_different_type() {
        let h1 = content_hash("test", &ContentType::Text);
        let h2 = content_hash("test", &ContentType::Image);
        assert_ne!(h1, h2);
    }
}
