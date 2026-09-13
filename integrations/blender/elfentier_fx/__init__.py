bl_info = {
    "name": "ElfentierFX",
    "author": "elfentierFX contributors",
    "version": (0, 1, 0),
    "blender": (4, 0, 0),
    "location": "File > Import > ElfentierFX Bundle",
    "description": "Import elfentierFX cook export bundles (GLB mesh, liquid cache point clouds)",
    "category": "Import-Export",
}

from . import import_bundle

def register():
    import_bundle.register()


def unregister():
    import_bundle.unregister()
