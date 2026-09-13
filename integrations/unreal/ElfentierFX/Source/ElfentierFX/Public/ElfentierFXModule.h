#pragma once

#include "CoreMinimal.h"
#include "Modules/ModuleInterface.h"

/** Editor module for elfentierFX bundle import utilities. Live TCP bridge: future work. */
class FElfentierFXModule : public IModuleInterface
{
public:
    virtual void StartupModule() override;
    virtual void ShutdownModule() override;
};
