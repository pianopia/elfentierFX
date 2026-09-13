import type { ViewportEnvironment } from "../types/graph";

const STORAGE_KEY = "elfentier-viewport-environment";

export const DEFAULT_VIEWPORT_ENVIRONMENT: ViewportEnvironment = {
  preset: "studio_soft",
  hdr_path: null,
  intensity: 1.0,
  rotation_yaw_deg: 0,
  diffuse_blur: 0.35,
  enabled: true,
};

export function loadViewportEnvironment(): ViewportEnvironment {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_VIEWPORT_ENVIRONMENT };
    const parsed = JSON.parse(raw) as Partial<ViewportEnvironment>;
    return {
      ...DEFAULT_VIEWPORT_ENVIRONMENT,
      ...parsed,
    };
  } catch {
    return { ...DEFAULT_VIEWPORT_ENVIRONMENT };
  }
}

export function saveViewportEnvironment(env: ViewportEnvironment): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(env));
}
