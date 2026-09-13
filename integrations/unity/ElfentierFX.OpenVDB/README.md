# Unity volume import (moved)

elfentierFX no longer ships a Unity volume import package from this repository.

For **3D volume playback** in Unity — including legacy `volume_texture.evol` (`elfentier_volume_texture_v1`) from elfentierFX export bundles — install the separate product:

**[Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter)**

| | |
|---|---|
| UPM package | `com.louddin.unity-volume-importer` |
| Install (Git URL) | `https://github.com/pianopia/UnityVolumeImporter.git?path=Packages/com.louddin.unity-volume-importer` |

## elfentierFX interchange formats (unchanged)

- **Export bundles** — smoke graphs include `volume_texture.evol` alongside the XY atlas. Import the bundle with **ElfentierFX → Import Export Bundle…** (`integrations/unity/ElfentierFX/`).
- **CLI** — `tools/vdb_convert` reads and writes `.evol` for engine interchange.

OpenVDB is a trademark of LF Projects, LLC.
