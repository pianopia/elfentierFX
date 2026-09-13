#include "ElfentierLiquidCacheLoader.h"
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"

namespace
{
    bool ParseHeader(const FString& HeaderLine, int32& OutFrameCount, int32& OutParticlesPerFrame)
    {
        OutFrameCount = 0;
        OutParticlesPerFrame = 0;

        if (!HeaderLine.StartsWith(TEXT("# frames=")))
        {
            return false;
        }

        TArray<FString> Tokens;
        HeaderLine.ParseIntoArrayWS(Tokens);
        for (const FString& Token : Tokens)
        {
            if (Token.StartsWith(TEXT("frames=")))
            {
                OutFrameCount = FCString::Atoi(*Token.Mid(7));
            }
            else if (Token.StartsWith(TEXT("particles_per_frame=")))
            {
                OutParticlesPerFrame = FCString::Atoi(*Token.Mid(20));
            }
        }

        return OutFrameCount > 0 && OutParticlesPerFrame > 0;
    }
}

bool FElfentierLiquidCacheLoader::LoadFromFile(const FString& FilePath, TArray<FElfentierLiquidFrame>& OutFrames)
{
    OutFrames.Reset();

    TArray<uint8> RawBytes;
    if (!FFileHelper::LoadFileToArray(RawBytes, *FilePath))
    {
        return false;
    }

    int32 DataOffset = 0;
    int32 FrameCount = 0;
    int32 ParticlesPerFrame = 0;

    FString HeaderAccum;
    for (int32 i = 0; i < RawBytes.Num(); ++i)
    {
        const char C = static_cast<char>(RawBytes[i]);
        if (C == '\n')
        {
            if (HeaderAccum.StartsWith(TEXT("#")))
            {
                ParseHeader(HeaderAccum, FrameCount, ParticlesPerFrame);
                DataOffset = i + 1;
                HeaderAccum.Reset();
                continue;
            }
            break;
        }

        if (C == '\r')
        {
            continue;
        }

        HeaderAccum.AppendChar(TCHAR(C));
    }

    if (FrameCount <= 0 || ParticlesPerFrame <= 0)
    {
        return false;
    }

    const int32 FloatsPerParticle = 4;
    const int32 BytesPerFrame = ParticlesPerFrame * FloatsPerParticle * static_cast<int32>(sizeof(float));
    if (DataOffset + FrameCount * BytesPerFrame > RawBytes.Num())
    {
        return false;
    }

    OutFrames.Reserve(FrameCount);
    int32 Offset = DataOffset;

    for (int32 FrameIndex = 0; FrameIndex < FrameCount; ++FrameIndex)
    {
        FElfentierLiquidFrame Frame;
        Frame.Particles.Reserve(ParticlesPerFrame);

        for (int32 ParticleIndex = 0; ParticleIndex < ParticlesPerFrame; ++ParticleIndex)
        {
            float Values[4];
            FMemory::Memcpy(Values, RawBytes.GetData() + Offset, sizeof(Values));
            Offset += sizeof(Values);

            FElfentierLiquidParticle Particle;
            // Source: meters, Y-up. Unreal: centimeters, Z-up — caller applies rotation/scale.
            Particle.Position = FVector(Values[0] * 100.f, Values[2] * 100.f, Values[1] * 100.f);
            Particle.Radius = Values[3] * 100.f;
            Frame.Particles.Add(Particle);
        }

        OutFrames.Add(MoveTemp(Frame));
    }

    return true;
}
