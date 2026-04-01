fn decode_percent_encoded(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut decoded = Vec::with_capacity(bytes.len());

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);

            if let (Some(high), Some(low)) = (high, low) {
                decoded.push(((high << 4) | low) as u8);
                index += 3;
                continue;
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn normalize_file_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_uri_prefix = trimmed
        .strip_prefix("file://localhost")
        .or_else(|| trimmed.strip_prefix("file://"))
        .unwrap_or(trimmed);

    Some(decode_percent_encoded(without_uri_prefix))
}

pub fn normalize_file_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| normalize_file_path(path))
        .collect()
}

pub fn serialize_file_paths(paths: &[String]) -> Result<String, serde_json::Error> {
    serde_json::to_string(&normalize_file_paths(paths))
}

pub fn deserialize_file_paths(content: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(content).map_or_else(
        |_| normalize_file_paths(&[content.to_string()]),
        |paths| normalize_file_paths(&paths),
    )
}

pub fn hash_file_paths(paths: &[String]) -> u64 {
    serialize_file_paths(paths).map_or(0, |serialized| seahash::hash(serialized.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_file_paths_strips_uri_prefix_and_decodes_percent_encoding() {
        let paths = vec![
            "file:///tmp/hello%20world.txt".to_string(),
            "file://localhost/tmp/demo.txt".to_string(),
        ];

        let normalized = normalize_file_paths(&paths);

        assert_eq!(normalized, vec!["/tmp/hello world.txt", "/tmp/demo.txt"]);
    }

    #[test]
    fn test_normalize_file_paths_drops_blank_entries() {
        let paths = vec![String::new(), "  ".to_string(), "/tmp/demo.txt".to_string()];

        let normalized = normalize_file_paths(&paths);

        assert_eq!(normalized, vec!["/tmp/demo.txt"]);
    }

    #[test]
    fn test_serialize_file_paths_returns_json_array_of_normalized_paths() {
        let serialized = serialize_file_paths(&[
            "file:///tmp/alpha.txt".to_string(),
            "/tmp/beta.txt".to_string(),
        ])
        .unwrap_or_else(|error| panic!("serialize should succeed: {error}"));

        assert_eq!(serialized, "[\"/tmp/alpha.txt\",\"/tmp/beta.txt\"]");
    }

    #[test]
    fn test_deserialize_file_paths_when_json_array_returns_normalized_paths() {
        let deserialized = deserialize_file_paths("[\"file:///tmp/alpha.txt\",\"/tmp/beta.txt\"]");

        assert_eq!(deserialized, vec!["/tmp/alpha.txt", "/tmp/beta.txt"]);
    }

    #[test]
    fn test_deserialize_file_paths_when_legacy_string_returns_single_path() {
        let deserialized = deserialize_file_paths("/tmp/legacy.txt");

        assert_eq!(deserialized, vec!["/tmp/legacy.txt"]);
    }

    #[test]
    fn test_hash_file_paths_is_stable_for_equivalent_normalized_inputs() {
        let hash_a = hash_file_paths(&["file:///tmp/demo%20file.txt".to_string()]);
        let hash_b = hash_file_paths(&["/tmp/demo file.txt".to_string()]);

        assert_eq!(hash_a, hash_b);
    }
}
