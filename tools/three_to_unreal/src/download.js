import { zipSync } from 'fflate';
import { exportThreeBundle } from './exporter.js';

/** Call from a user gesture in the source Three.js app after images have loaded. */
export async function downloadThreeBundle(scene, options = {}) {
  const bundle = await exportThreeBundle(scene, options);
  const bytes = zipSync(Object.fromEntries(bundle.files), { level: 1 });
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
  const link = document.createElement('a');
  link.href = url; link.download = bundle.manifest.name + '.zip';
  document.body.appendChild(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 30000);
  return bundle.manifest;
}
