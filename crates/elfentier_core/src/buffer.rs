//! Host-facing buffer types for mesh and volume data.

use serde::{Deserialize, Serialize};

/// A typed byte buffer owned by the core.
///
/// Alpha 0 stub — actual allocation and FFI handoff come in later milestones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Buffer {
    pub label: String,
    pub byte_len: usize,
}

impl Buffer {
    pub fn empty(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            byte_len: 0,
        }
    }
}
