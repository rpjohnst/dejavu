cbuffer View : register(b0) {
    float2 view_size;
    float2 port_size;
};

struct Vertex {
    float3 position : POSITION;
    float2 uv : TEXCOORD0;
    float4 image : TEXCOORD1;
};

struct VertexOut {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 image : TEXCOORD1;
};

VertexOut main(Vertex vertex) {
    VertexOut output;
    output.position = float4(
        vertex.position.x * 2.0 / view_size.x - 1.0,
        vertex.position.y * -2.0 / view_size.y + 1.0,
        vertex.position.z,
        1.0
    );
    output.uv = vertex.uv;
    output.image = vertex.image;

    // D3D10 and later sample the viewport at pixel centers, while GM uses D3D8
    // which samples the viewport at the upper-left corners of pixels. Emulate
    // the older behavior by offsetting clip space by a half pixel.
    output.position.xy += float2(1.0, -1.0) / port_size * output.position.w;

    return output;
}
