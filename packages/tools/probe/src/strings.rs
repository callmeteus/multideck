//! Printable-string extraction for binary blobs.
//!
//! This is deliberately naive: it walks the file byte by byte and keeps runs of
//! printable ASCII. That is enough to pull hostnames, mDNS service names, JSON
//! keys and error messages out of a stripped executable without pulling in a
//! dependency.

/// Default minimum run length reported by `probe strings`.
pub const DEFAULT_MIN_LEN: usize = 6;

/// Default cap on the number of printed lines.
pub const DEFAULT_LIMIT: usize = 200;

/// One printable ASCII run found inside a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Byte offset of the first character of the run.
    pub offset: usize,
    /// The run, decoded as ASCII.
    pub text: String,
}

/// Returns true for bytes that render as themselves in a terminal.
///
/// Only printable ASCII (`0x20..=0x7E`) counts. Control bytes, including tab and
/// newline, terminate a run so that each reported string stays on one line.
pub fn is_printable(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

/// Extracts every printable ASCII run of at least `min_len` bytes.
///
/// `min_len` is clamped to 1, since a zero-length run is never useful. Runs are
/// returned in file order, with the byte offset of their first character.
pub fn extract(bytes: &[u8], min_len: usize) -> Vec<Found> {
    let min_len = min_len.max(1);
    let mut found = Vec::new();
    let mut start: Option<usize> = None;

    for (index, &byte) in bytes.iter().enumerate() {
        if is_printable(byte) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(begin) = start.take() {
            push_run(&mut found, bytes, begin, index, min_len);
        }
    }

    // A run that reaches the end of the file has no terminator after it.
    if let Some(begin) = start {
        push_run(&mut found, bytes, begin, bytes.len(), min_len);
    }

    found
}

/// Appends the run `bytes[begin..end]` when it is long enough.
fn push_run(found: &mut Vec<Found>, bytes: &[u8], begin: usize, end: usize, min_len: usize) {
    if end - begin < min_len {
        return;
    }
    let text = String::from_utf8_lossy(&bytes[begin..end]).into_owned();
    found.push(Found {
        offset: begin,
        text,
    });
}

/// Case-insensitive substring filter used by `--grep`.
///
/// An empty needle matches everything, so `--grep ""` behaves like no filter.
pub fn matches_grep(text: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    text.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_single_run_and_its_offset() {
        let hits = extract(b"\x00\x01elgato\x00", 3);
        assert_eq!(
            hits,
            vec![Found {
                offset: 2,
                text: "elgato".to_string(),
            }]
        );
    }

    #[test]
    fn runs_shorter_than_min_are_dropped() {
        let hits = extract(b"abc\x00abcdef", 4);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "abcdef");
        assert_eq!(hits[0].offset, 4);
    }

    #[test]
    fn run_at_end_of_file_is_reported() {
        let hits = extract(b"\x00_cora._udp", 6);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "_cora._udp");
        assert_eq!(hits[0].offset, 1);
    }

    #[test]
    fn non_ascii_bytes_split_runs() {
        let hits = extract("ab\u{00e9}cd".as_bytes(), 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].text, "ab");
        assert_eq!(hits[1].text, "cd");
        // `é` is two bytes (0xC3 0xA9), so `cd` starts at offset 4.
        assert_eq!(hits[1].offset, 4);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(extract(b"", 1).is_empty());
    }

    #[test]
    fn min_len_zero_is_clamped_to_one() {
        let hits = extract(b"\x00A\x00", 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "A");
    }

    #[test]
    fn grep_is_case_insensitive() {
        assert!(matches_grep("Stream Deck Mobile", "mobile"));
        assert!(matches_grep("CORA", "cora"));
        assert!(!matches_grep("sdmobile", "elgato"));
        assert!(matches_grep("anything", ""));
    }
}
