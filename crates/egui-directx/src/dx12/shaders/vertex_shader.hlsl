struct VertexInput
{
    float2 pos: POSITION;
    float2 uv: TEXCOORD0;
    float4 color: COLOR;
};

struct VertexOutput
{
    float4 pos : SV_POSITION;
    float2 uv: TEXCOORD0;
    float4 color: COLOR;
};

struct Constants
{
    float2 screenSize;
};

ConstantBuffer<Constants> g_constants : register(b0, space0);

VertexOutput main(VertexInput vertex)
{
    VertexOutput o;
    o.pos = float4(2.0 * vertex.pos.x / g_constants.screenSize.x - 1.0, 1.0 - 2.0 * vertex.pos.y / g_constants.screenSize.y, 0.0, 1.0);
    o.uv = vertex.uv;
    o.color = vertex.color;
    return o;
}