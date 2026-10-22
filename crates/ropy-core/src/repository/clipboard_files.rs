const fn hex_digit_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_percent_encoded(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut decoded = Vec::with_capacity(bytes.len());

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = hex_digit_value(bytes[index + 1]);
            let low = hex_digit_value(bytes[index + 2]);

            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy_owned(decoded)
}

fn normalize_file_path(path: &str) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }

    // Percent escapes belong to URIs; whitespace and percent characters in a
    // filesystem path are part of its identity.
    let Some(uri) = path.strip_prefix("file://") else {
        return Some(path.to_string());
    };
    let path_start = uri.find('/')?;
    let authority = &uri[..path_start];
    if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
        // A remote URI cannot be treated as a relative path on this machine.
        return None;
    }
    Some(decode_percent_encoded(&uri[path_start..]))
}

/// Decode local file URIs, ignoring remote authorities and blank entries.
/// Literal filesystem whitespace and percent characters are preserved.
#[must_use]
pub fn normalize_file_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| normalize_file_path(path))
        .collect()
}

/// Encode normalized file paths as a JSON array.
///
/// # Errors
/// Returns a JSON serialization error if the normalized paths cannot be encoded.
pub fn serialize_file_paths(paths: &[String]) -> Result<String, serde_json::Error> {
    serde_json::to_string(&normalize_file_paths(paths))
}

/// Decode a file-list record, accepting a legacy single-path value.
#[must_use]
pub fn deserialize_file_paths(content: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(content).map_or_else(
        |_| normalize_file_paths(&[content.to_string()]),
        |paths| normalize_file_paths(&paths),
    )
}

/// Hash the normalized JSON file list for capture deduplication.
#[must_use]
pub fn hash_file_paths(paths: &[String]) -> u64 {
    serialize_file_paths(paths).map_or(0, |serialized| seahash::hash(serialized.as_bytes()))
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case("/tmp/report ", "/tmp/report ")]
    #[case("/tmp/report\t", "/tmp/report\t")]
    #[case("file:///tmp/report%20", "/tmp/report ")]
    #[case("file:///tmp/report%09", "/tmp/report\t")]
    fn test_file_paths_significant_whitespace_survives_round_trip(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        let normalized = normalize_file_paths(&[input.into()]);
        assert_eq!(normalized, vec![expected]);
        assert_eq!(normalize_file_paths(&normalized), normalized);
        let serialized =
            serialize_file_paths(&normalized).unwrap_or_else(|error| panic!("serialize: {error}"));
        assert_eq!(deserialize_file_paths(&serialized), normalized);
    }

    #[rstest::rstest]
    #[case("/tmp/report%20final.txt")]
    #[case("/tmp/report%25final.txt")]
    #[case("/tmp/report%2520final.txt")]
    fn test_file_paths_literal_percent_encoding_round_trip_preserves_path(#[case] path: &str) {
        let paths = vec![path.to_string()];
        assert_eq!(normalize_file_paths(&paths), paths);
        let serialized = serialize_file_paths(&paths)
            .unwrap_or_else(|error| panic!("serialize should succeed: {error}"));
        assert_eq!(deserialize_file_paths(&serialized), paths);
    }

    #[test]
    fn test_normalize_file_paths_encoded_percent_uri_repeated_calls_preserve_path() {
        let normalized = normalize_file_paths(&["file:///tmp/report%2520final.txt".into()]);
        assert_eq!(normalized, vec!["/tmp/report%20final.txt"]);
        assert_eq!(normalize_file_paths(&normalized), normalized);
    }

    #[test]
    fn test_normalize_file_paths_strips_uri_prefix_and_decodes_percent_encoding() {
        let paths = vec![
            "file:///tmp/hello%20world.txt".to_string(),
            "file://localhost/tmp/demo.txt".to_string(),
            "file://LOCALHOST/tmp/case.txt".to_string(),
        ];

        let normalized = normalize_file_paths(&paths);

        assert_eq!(
            normalized,
            vec!["/tmp/hello world.txt", "/tmp/demo.txt", "/tmp/case.txt"]
        );
    }

    #[rstest::rstest]
    #[case("file://server/share/report.txt")]
    #[case("file://localhost.example/tmp/report.txt")]
    #[case("file://localhost")]
    #[case("file://relative")]
    fn test_normalize_file_paths_unsupported_authority_is_not_a_local_path(#[case] uri: &str) {
        let paths = vec![uri.to_string()];
        assert_eq!(normalize_file_paths(&paths), Vec::<String>::new());
        let serialized = serde_json::to_string(&paths)
            .unwrap_or_else(|error| panic!("serialize fixture: {error}"));
        assert_eq!(deserialize_file_paths(&serialized), Vec::<String>::new());
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
