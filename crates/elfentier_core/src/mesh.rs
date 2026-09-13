//! Mesh representation stub.

use crate::buffer::Buffer;

/// A polygon mesh with separate vertex and index buffers.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub name: String,
    pub vertices: Buffer,
    pub indices: Buffer,
}

impl Mesh {
    pub fn unnamed() -> Self {
        Self {
            name: String::new(),
            vertices: Buffer::empty("positions"),
            indices: Buffer::empty("indices"),
        }
    }
}
