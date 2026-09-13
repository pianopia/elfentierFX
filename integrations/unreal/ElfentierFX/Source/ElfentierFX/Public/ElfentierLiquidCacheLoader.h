#pragma once

#include "CoreMinimal.h"

struct FElfentierLiquidParticle
{
    FVector Position = FVector::ZeroVector;
    float Radius = 0.1f;
};

struct FElfentierLiquidFrame
{
    TArray<FElfentierLiquidParticle> Particles;
};

/** Parses elfentier_liquid_cache_v1 raw files (meters, Y-up source). */
class ELFENTIERFX_API FElfentierLiquidCacheLoader
{
public:
    static bool LoadFromFile(const FString& FilePath, TArray<FElfentierLiquidFrame>& OutFrames);
};
