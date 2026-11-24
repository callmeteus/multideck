//! Typed failures for domain operations.

use thiserror::Error;

/// Errors produced by the domain layer.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A key index is outside the device grid.
    #[error("key {key} is outside a {cols}x{rows} grid")]
    InvalidKey {
        /// Requested key index.
        key: u8,
        /// Grid columns.
        cols: u8,
        /// Grid rows.
        rows: u8,
    },

    /// A setting required by an action is missing or has the wrong type.
    #[error("action '{kind}' is missing setting '{field}'")]
    MissingSetting {
        /// Action kind.
        kind: String,
        /// Setting key.
        field: String,
    },

    /// A profile could not be decoded or validated.
    #[error("invalid profile: {0}")]
    InvalidProfile(String),
}
