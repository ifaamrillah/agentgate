//! Kode yang dipakai bersama oleh server dan CLI Agent Gate: type domain bersama,
//! parser workflow, dan protokol runner.

/// Versi Agent Gate. Server dan CLI melaporkan versi yang sama.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod persona;
pub mod runner;
pub mod workflow;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
