/// Shared test utilities for fixture and integration testing.
///
/// This crate is scaffold-only for now.
pub fn test_util_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn util_is_ready() {
        assert!(super::test_util_ready());
    }
}
