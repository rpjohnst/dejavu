cbuffer Material : register(b0) {
    float2 atlas_size;
};

Texture2D tex;
SamplerState samp;

struct PixelIn {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
    nointerpolation float4 image : TEXCOORD1;
};

float4 main(PixelIn input) : SV_TARGET {
    // Emulate clamped texture sampling within the atlas texture.
    float2 wh = input.image.zw;
    float2 st = clamp(input.uv * wh, float2(0.5, 0.5), wh - float2(0.5, 0.5));
    float2 uv = (input.image.xy + st) / atlas_size;

    return tex.Sample(samp, uv);
}
