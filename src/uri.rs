//! Helpers for LSP `file://` URIs.

use std::path::PathBuf;

/// Convert a `file://` URI into a local filesystem path.
///
/// Performs percent-decoding so URIs like `file:///tmp/my%20file.rs` map to
/// `/tmp/my file.rs`. Hostnames (`file://localhost/...`) are stripped.
pub fn path_from_file_uri(uri: &str) -> Result<PathBuf, String> {
    let rest = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("URI must start with file://, got: {uri}"))?;

    let path_part = if rest.starts_with('/') {
        rest
    } else {
        // file://hostname/path → skip host
        rest.find('/')
            .map(|i| &rest[i..])
            .ok_or_else(|| format!("file URI has no path: {uri}"))?
    };

    let decoded = percent_decode(path_part)?;
    Ok(PathBuf::from(decoded))
}

/// Percent-decode a path string (`%20` → space, `%2F` → `/`, etc.).
fn percent_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = from_hex(bytes[i + 1])?;
                let l = from_hex(bytes[i + 2])?;
                out.push((h << 4) | l);
                i += 3;
            }
            b'+' => {
                // Paths use %20 for spaces; treat '+' literally.
                out.push(b'+');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|e| format!("percent-decoded path is not UTF-8: {e}"))
}

fn from_hex(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid percent-encoding hex digit: {}", b as char)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_space() {
        let p = path_from_file_uri("file:///tmp/my%20file.rs").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/my file.rs"));
    }

    #[test]
    fn plain_path_unchanged() {
        let p = path_from_file_uri("file:///home/user/main.rs").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/main.rs"));
    }

    #[test]
    fn strips_localhost() {
        let p = path_from_file_uri("file://localhost/tmp/a%20b.rs").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/a b.rs"));
    }

    #[test]
    fn rejects_non_file() {
        assert!(path_from_file_uri("http://example.com/x").is_err());
    }
}
