# ElfentierFX Blender add-on

Installable Blender 4.x add-on for importing elfentierFX cook export bundles.

## Install

1. Zip the `elfentier_fx` folder **or** copy it into your Blender scripts path.
2. In Blender: **Edit → Preferences → Add-ons → Install…**
3. Select the folder/zip and enable **ElfentierFX**.

Alternatively, symlink:

```bash
ln -s /path/to/elfentierFX/integrations/blender/elfentier_fx \
  ~/.config/blender/4.2/scripts/addons/elfentier_fx
```

## Import

1. Cook a graph in elfentierFX and click **Export Bundle**.
2. In Blender: **File → Import → ElfentierFX Bundle**
3. Select the bundle directory containing `manifest.json`.

### Graph modes

| Mode | Result |
|------|--------|
| `city` | Imports `city_mesh.glb` via glTF 2.0 importer |
| `liquid` | Point cloud mesh + shape keys per frame at manifest fps |
| `smoke` | Placeholder text object with atlas path (manual flipbook) |

## Requirements

- Blender 4.0+
- Built-in **glTF 2.0** importer enabled (default)

## Format reference

[`docs/export-formats.md`](../../../docs/export-formats.md)

## Limitations (Phase 1)

- Smoke atlas is a stub marker, not a volume texture
- Liquid uses shape-key playback, not molecular / fluid sim
- No live bridge to running elfentierFX
