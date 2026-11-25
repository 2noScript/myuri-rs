//! Rust port of `JWriter20/Myuri` 5.1.0 and `JWriter20/cursory-js` 2.1.0.

/// Crate version aligned with upstream `@jwriter20/myuri` 5.1.0.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_myuri_upstream() {
        assert_eq!(VERSION, "5.1.0");
    }
}
