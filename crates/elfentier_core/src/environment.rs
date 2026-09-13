//! Viewport HDR / image-based environment settings (shared between UI and wgpu renderer).

use serde::{Deserialize, Serialize};

/// Built-in or custom environment source for the native wgpu viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportEnvironmentPreset {
    /// Legacy flat gray clear color (no HDR).
    FlatGray,
    /// Soft studio gradient with warm key light — default for smoke lookdev.
    StudioSoft,
    /// Higher-contrast studio with darker sides and bright overhead.
    StudioContrast,
    /// Load from `hdr_path` (.hdr / .exr / PNG / JPEG equirectangular).
    Custom,
}

impl Default for ViewportEnvironmentPreset {
    fn default() -> Self {
        Self::StudioSoft
    }
}

/// HDR / IBL settings for the native viewport preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportEnvironment {
    pub preset: ViewportEnvironmentPreset,
    /// Filesystem path when `preset` is `Custom`.
    pub hdr_path: Option<String>,
    /// Exposure multiplier applied to environment radiance.
    pub intensity: f32,
    /// Y-axis rotation in degrees (yaw).
    pub rotation_yaw_deg: f32,
    /// Diffuse environment blur (0 = sharp, 1 = heavy blur).
    pub diffuse_blur: f32,
    /// When false, falls back to flat gray clear color.
    pub enabled: bool,
}

impl Default for ViewportEnvironment {
    fn default() -> Self {
        Self {
            preset: ViewportEnvironmentPreset::StudioSoft,
            hdr_path: None,
            intensity: 1.0,
            rotation_yaw_deg: 0.0,
            diffuse_blur: 0.35,
            enabled: true,
        }
    }
}

impl ViewportEnvironment {
    /// Legacy flat-gray background matching pre-HDR viewport behavior.
    pub fn legacy_flat() -> Self {
        Self {
            preset: ViewportEnvironmentPreset::FlatGray,
            hdr_path: None,
            intensity: 1.0,
            rotation_yaw_deg: 0.0,
            diffuse_blur: 0.0,
            enabled: false,
        }
    }

    /// Effective environment for rendering (disabled presets map to flat gray).
    pub fn effective_preset(&self) -> ViewportEnvironmentPreset {
        if !self.enabled {
            ViewportEnvironmentPreset::FlatGray
        } else {
            self.preset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_environment_is_studio_soft() {
        let env = ViewportEnvironment::default();
        assert_eq!(env.preset, ViewportEnvironmentPreset::StudioSoft);
        assert!(env.enabled);
    }

    #[test]
    fn disabled_environment_maps_to_flat_gray() {
        let env = ViewportEnvironment {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(
            env.effective_preset(),
            ViewportEnvironmentPreset::FlatGray
        );
    }
}
