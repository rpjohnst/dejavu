Texture2D tex;
SamplerState samp;

struct PixelIn {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD0;
};

float4 main(PixelIn input) : SV_TARGET {
    return tex.Sample(samp, input.tex);
}
