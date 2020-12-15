cbuffer View : register(b0) {
    float2 view_size;
};

struct Vertex {
    float3 pos : POSITION;
    float2 tex : TEXCOORD0;
};

struct VertexOut {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD0;
};

VertexOut main(Vertex vertex) {
    VertexOut output;
    output.pos = float4(
        vertex.pos.x * 2.0 / view_size.x - 1.0,
        vertex.pos.y * -2.0 / view_size.y + 1.0,
        vertex.pos.z,
        1.0
    );
    output.tex = vertex.tex;
    return output;
}
