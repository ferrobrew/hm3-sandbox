SamplerState g_sampler : register(s1, space0);
Texture2D g_texture : register(t2, space0);

struct VertexInput
{
    float4 pos: SV_POSITION;
    float2 uv: TEXCOORD0;
    float4 color: COLOR;
};

float4 main(VertexInput input) : SV_TARGET
{
    return input.color * g_texture.Sample(g_sampler, input.uv);
}