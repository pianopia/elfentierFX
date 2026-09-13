//! Mesh representation with positions and triangle indices.

use crate::buffer::Buffer;
use serde::{Deserialize, Serialize};

/// Three-component vector for positions and directions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn scale(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-6 {
            return Self::ZERO;
        }
        self.scale(1.0 / len)
    }
}

/// A polygon mesh with triangle list topology.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
    /// Legacy buffer handles for future FFI handoff.
    pub vertices: Buffer,
    pub indices_buffer: Buffer,
}

impl Mesh {
    pub fn unnamed() -> Self {
        Self {
            name: String::new(),
            positions: Vec::new(),
            indices: Vec::new(),
            vertices: Buffer::empty("positions"),
            indices_buffer: Buffer::empty("indices"),
        }
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::unnamed()
        }
    }

    pub fn vertex_count(&self) -> u32 {
        self.positions.len() as u32
    }

    pub fn triangle_count(&self) -> u32 {
        (self.indices.len() / 3) as u32
    }

    pub fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn add_quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        let base = self.positions.len() as u32;
        self.positions.extend([a, b, c, d]);
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn add_box(&mut self, min: Vec3, max: Vec3) {
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let base = self.positions.len() as u32;
        self.positions.extend(corners);
        let faces = [
            [0, 1, 2, 3],
            [5, 4, 7, 6],
            [4, 0, 3, 7],
            [1, 5, 6, 2],
            [3, 2, 6, 7],
            [4, 5, 1, 0],
        ];
        for face in faces {
            let i0 = base + face[0];
            let i1 = base + face[1];
            let i2 = base + face[2];
            let i3 = base + face[3];
            self.indices.extend([i0, i1, i2, i0, i2, i3]);
        }
    }

    pub fn transform(&self, position: Vec3, rotation_y: f32, scale: Vec3) -> Mesh {
        let cos = rotation_y.cos();
        let sin = rotation_y.sin();
        let positions = self
            .positions
            .iter()
            .map(|p| {
                let sx = p.x * scale.x;
                let sy = p.y * scale.y;
                let sz = p.z * scale.z;
                Vec3::new(
                    sx * cos - sz * sin + position.x,
                    sy + position.y,
                    sx * sin + sz * cos + position.z,
                )
            })
            .collect();
        Mesh {
            name: self.name.clone(),
            positions,
            indices: self.indices.clone(),
            vertices: self.vertices.clone(),
            indices_buffer: self.indices_buffer.clone(),
        }
    }

    pub fn merge(&mut self, other: &Mesh) {
        let offset = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self
            .indices
            .extend(other.indices.iter().map(|i| i + offset));
        if self.name.is_empty() {
            self.name = other.name.clone();
        }
    }
}

/// Creates a unit cube mesh (centered at origin, edge length 1).
pub fn create_unit_box_mesh() -> Mesh {
    let half = 0.5;
    let mut mesh = Mesh::with_name("unit_box");
    mesh.add_box(
        Vec3::new(-half, -half, -half),
        Vec3::new(half, half, half),
    );
    mesh.vertices.byte_len = mesh.positions.len() * 12;
    mesh.indices_buffer.byte_len = mesh.indices.len() * 4;
    mesh
}
