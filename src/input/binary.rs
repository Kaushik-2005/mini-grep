#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BinaryPolicy {
    #[default]
    Skip,
    Text,
}

impl BinaryPolicy {
    pub(crate) fn should_stop(self, bytes: &[u8]) -> bool {
        match self {
            Self::Skip => bytes.contains(&0),
            Self::Text => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_skipping_binary_content() {
        assert_eq!(BinaryPolicy::default(), BinaryPolicy::Skip);
    }

    #[test]
    fn bytes_without_nul_do_not_trigger_binary_detection() {
        let cases: &[&[u8]] = &[b"", b"Rust", b"Rust\r\n", "नमस्ते Rust".as_bytes(), b"\xff"];

        for bytes in cases {
            assert!(!BinaryPolicy::Skip.should_stop(bytes));
        }
    }

    #[test]
    fn detects_nul_at_any_position() {
        let cases: &[&[u8]] = &[b"\0", b"\0Rust", b"Ru\0st", b"Rust\0", b"\xff\0Rust"];

        for bytes in cases {
            assert!(BinaryPolicy::Skip.should_stop(bytes));
        }
    }

    #[test]
    fn text_policy_never_stops_for_nul() {
        let cases: &[&[u8]] = &[b"", b"Rust", b"\0", b"Ru\0st", b"\xff\0Rust"];

        for bytes in cases {
            assert!(!BinaryPolicy::Text.should_stop(bytes));
        }
    }
}
