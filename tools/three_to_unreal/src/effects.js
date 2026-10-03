import { FloatType, UnsignedByteType, RedFormat } from 'three';

function finiteArray(values, count, label) {
  if (!values || values.length !== count || !Array.from(values).every(Number.isFinite)) throw new Error(`Invalid ${label}`);
}

/** Extract a CPU-backed scalar Data3DTexture. GPU render targets require readback first. */
export function volumeFromTexture(texture, { boundsMin, boundsMax, name = 'density' }) {
  if (!texture?.isData3DTexture || texture.format !== RedFormat || ![FloatType, UnsignedByteType].includes(texture.type)) throw new Error('Expected scalar Float/UnsignedByte RedFormat Data3DTexture');
  const { data, width, height, depth } = texture.image;
  const frame = Float32Array.from(data, n => texture.type === UnsignedByteType ? n / 255 : n);
  const volume = { name, resolution: [width, height, depth], boundsMin, boundsMax, frames: [frame] };
  encodeVolume(volume); // Same bounds/layout/finite validation as export.
  return volume;
}

export function encodeVolume({ resolution, boundsMin, boundsMax, frames }) {
  finiteArray(resolution, 3, 'volume resolution');
  if (!resolution.every(n => Number.isSafeInteger(n) && n > 0 && n <= 512)) throw new Error('Resolution must be 1..512');
  finiteArray(boundsMin, 3, 'boundsMin'); finiteArray(boundsMax, 3, 'boundsMax');
  if (boundsMax.some((n, i) => n <= boundsMin[i])) throw new Error('Volume bounds must have positive extents');
  if (!frames?.length) throw new Error('Volume needs at least one frame');
  const cells = resolution.reduce((a, b) => a * b, 1);
  const byteLength = 52 + cells * frames.length * 4;
  if (byteLength > 512 * 1024 * 1024) throw new Error('Volume exceeds 512 MiB; split into smaller sequences');
  const bytes = new Uint8Array(byteLength);
  bytes.set([69, 70, 86, 84]);
  const view = new DataView(bytes.buffer);
  [1, ...resolution, frames.length, 1].forEach((v, i) => view.setUint32(4 + i * 4, v, true));
  [...boundsMin, ...boundsMax].forEach((v, i) => view.setFloat32(28 + i * 4, v, true));
  frames.forEach((frame, f) => {
    finiteArray(frame, cells, `volume frame ${f}`);
    if (Array.from(frame).some(v => v < 0)) throw new Error('Fog density must be nonnegative');
    for (let i = 0; i < cells; i++) view.setFloat32(52 + (f * cells + i) * 4, frame[i], true);
  });
  return bytes;
}

// Fixed topology surface snapshots, suitable for baked FLIP/water or GPU readback.
export function encodeSurfaceCache({ frames, indices, fps = 24 }) {
  if (!frames?.length || !Number.isFinite(fps) || fps <= 0 || fps > 240) throw new Error('Invalid cache frames/fps');
  const count = frames[0].length;
  if (!count || count % 3 || count * frames.length > 32_000_000) throw new Error('Invalid/oversized surface cache');
  if (!indices?.length || indices.length % 3 || !Array.from(indices).every(i => Number.isInteger(i) && i >= 0 && i < count / 3)) throw new Error('Invalid triangle indices');
  frames.forEach((frame, i) => finiteArray(frame, count, `surface frame ${i}`));
  return { format: 'elfentier_surface_cache_v1', units: 'meters', up_axis: 'Y', fps,
    indices: Array.from(indices), frames: frames.map(f => Array.from(f)) };
}

// Analytic waves ported to a native material; this does not reproduce a fluid solver.
export function waterRecipe(config, name) {
  const waves = config.waves ?? [{ direction: [1, 0], amplitude: 0.15, wavelength: 4, speed: 1 }];
  if (!waves.length || waves.length > 16) throw new Error('Water supports 1..16 waves');
  const lines = ['float3 offset = float3(0,0,0);'];
  waves.forEach((wave, i) => {
    finiteArray(wave.direction, 2, 'wave direction');
    const norm = Math.hypot(...wave.direction);
    if (norm === 0 || ![wave.amplitude, wave.wavelength, wave.speed].every(Number.isFinite) || wave.wavelength <= 0 || wave.amplitude < 0) throw new Error('Invalid wave parameters');
    const [x, z] = wave.direction.map(n => n / norm);
    // UE Interchange glTF: source (x,y,z) -> (x,z,y), meters -> cm.
    lines.push(`float phase${i} = ${2 * Math.PI / wave.wavelength} * dot(Position.xy / 100.0, float2(${x}, ${z})) - Time * ${wave.speed};`);
    lines.push(`offset.z += ${wave.amplitude * 100} * sin(phase${i});`);
  });
  lines.push('return offset;');
  const color = config.color ?? [0.015, 0.12, 0.16]; finiteArray(color, 3, 'water color');
  const roughness = config.roughness ?? 0.08;
  if (!Number.isFinite(roughness) || roughness < 0 || roughness > 1) throw new Error('Invalid water roughness');
  return { name, kind: 'water_wpo', hlsl: lines.join('\n'), output: 'float3',
    inputs: [{ name: 'Position', kind: 'world_position' }, { name: 'Time', kind: 'time' }], color, roughness,
    note: 'Opaque lit surface + world-space sine-wave WPO. Refraction/foam/solver are not reproduced.' };
}
