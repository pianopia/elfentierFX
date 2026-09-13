#include "ElfentierFXModule.h"
#include "Modules/ModuleManager.h"

#define LOCTEXT_NAMESPACE "FElfentierFXModule"

void FElfentierFXModule::StartupModule()
{
    UE_LOG(LogTemp, Log, TEXT("ElfentierFX module loaded"));
}

void FElfentierFXModule::ShutdownModule()
{
}

#undef LOCTEXT_NAMESPACE

IMPLEMENT_MODULE(FElfentierFXModule, ElfentierFX)
