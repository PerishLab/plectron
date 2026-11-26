cbuffer Frame : register(b0) {
    float4 first;
    float4 second;
    float2 origin;
    float2 extra;
};

ByteAddressBuffer verts : register(t0);
ByteAddressBuffer data : register(t1);
Texture2D<float4> source : register(t2);

struct Vertex {
    float4 position : SV_Position;
    float3 color : COLOR0;
    nointerpolation uint path : TEXCOORD0;
};

float snap(float value) {
    return floor(value + 0.5);
}

float3 snap3(float3 value) {
    return floor(value + 0.5);
}

float4 snap4(float4 value) {
    return floor(value + 0.5);
}

Vertex vertexMain(uint index : SV_VertexID) {
    uint base = index * 20;
    float2 pos = asfloat(verts.Load2(base));
    float3 col = asfloat(verts.Load3(base + 8));
    Vertex result;
    result.position = float4(pos, 0.0, 1.0);
    result.color = col;
    result.path = 0;
    return result;
}

Vertex vertexPath(uint index : SV_VertexID, uint instance : SV_InstanceID) {
    uint base = instance * 32;
    float4 rect = asfloat(verts.Load4(base));
    float3 col = asfloat(verts.Load3(base + 16));
    float2 corners[6] = {
        float2(rect.x, rect.y),
        float2(rect.x, rect.w),
        float2(rect.z, rect.y),
        float2(rect.z, rect.y),
        float2(rect.x, rect.w),
        float2(rect.z, rect.w)
    };
    Vertex result;
    result.position = float4(corners[index], 0.0, 1.0);
    result.color = col;
    result.path = instance;
    return result;
}

float4 fragmentMain(Vertex value) : SV_Target {
    return float4(value.color, 1.0);
}

float4 fragmentComposite(Vertex value) : SV_Target {
    int2 coord = int2(value.position.xy + first.xy);
    return source.Load(int3(coord, 0)) * value.color.r;
}

float4 fragmentPath(Vertex value) : SV_Target {
    uint head = value.path * 32;
    uint4 meta = data.Load4(head);
    float4 clip = asfloat(data.Load4(head + 16));
    uint edgebase = meta.x * 4;
    uint bandbase = meta.y * 4;
    int rel = int(floor(value.position.y) + origin.y) - int(meta.z);
    if (rel < 0 || meta.w == 0) {
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    uint band = uint(rel) / meta.w;
    uint firstoff = data.Load(bandbase);
    if (firstoff == 0 || band + 1 >= firstoff) {
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    uint begin = data.Load(bandbase + band * 4);
    uint end = data.Load(bandbase + (band + 1) * 4);
    uint samples = 0;
    float2 pixel = floor(value.position.xy) + origin;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 pt = pixel + (float2(column, row) + 0.5) / 4.0;
            if (pt.x < clip.x || pt.y < clip.y || pt.x >= clip.z || pt.y >= clip.w) {
                continue;
            }
            int winding = 0;
            for (uint i = begin; i < end; ++i) {
                uint idx = data.Load(bandbase + i * 4);
                float4 edge = asfloat(data.Load4(edgebase + idx * 16));
                if ((edge.y <= pt.y && edge.w > pt.y) || (edge.w <= pt.y && edge.y > pt.y)) {
                    float cross = edge.x + (pt.y - edge.y) / (edge.w - edge.y) * (edge.z - edge.x);
                    if (cross > pt.x) {
                        winding += edge.w > edge.y ? 1 : -1;
                    }
                }
            }
            samples += (winding != 0) ? 1 : 0;
        }
    }
    float alpha = floor((float(samples) * 255.0 + 8.0) / 16.0) / 255.0;
    return float4(snap3(value.color * alpha * 255.0) / 255.0, alpha);
}

float4 fragmentCurve(Vertex value) : SV_Target {
    uint head = value.path * 32;
    uint4 meta = data.Load4(head);
    float4 clip = asfloat(data.Load4(head + 16));
    uint curvebase = meta.x * 4;
    uint bandbase = meta.y * 4;
    int rel = int(floor(value.position.y) + origin.y) - int(meta.z);
    if (rel < 0 || meta.w == 0) {
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    uint band = uint(rel) / meta.w;
    uint firstoff = data.Load(bandbase);
    if (firstoff == 0 || band + 1 >= firstoff) {
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    uint begin = data.Load(bandbase + band * 4);
    uint end = data.Load(bandbase + (band + 1) * 4);
    uint samples = 0;
    float2 pixel = floor(value.position.xy) + origin;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 pt = pixel + (float2(column, row) + 0.5) / 4.0;
            if (pt.x < clip.x || pt.y < clip.y || pt.x >= clip.z || pt.y >= clip.w) {
                continue;
            }
            int winding = 0;
            for (uint i = begin; i < end; ++i) {
                uint idx = data.Load(bandbase + i * 4);
                float4 c0 = asfloat(data.Load4(curvebase + idx * 32));
                float4 c1 = asfloat(data.Load4(curvebase + idx * 32 + 16));
                if (pt.y < c0.y || pt.y >= c1.y) {
                    continue;
                }
                precise float dy = pt.y - c0.y;
                precise float cross;
                if (c1.w == 0.0) {
                    cross = c0.x + dy / (c1.y - c0.y) * (c1.x - c0.x);
                } else {
                    precise float b = c0.w - c0.y;
                    precise float a = (c1.y - c0.w) - b;
                    precise float disc = max(0.0, b * b + a * dy);
                    precise float root = sqrt(disc);
                    if (root > 0.0) {
                        root = 0.5 * (root + disc / root);
                    }
                    precise float divisor = b + root;
                    precise float q = dy / divisor;
                    precise float t = dy == 0.0 ? 0.0 : clamp(q + (dy - q * divisor) / divisor, 0.0, 1.0);
                    precise float left = c0.x + t * (c0.z - c0.x);
                    precise float right = c0.z + t * (c1.x - c0.z);
                    cross = left + t * (right - left);
                }
                if (cross > pt.x) {
                    winding += int(c1.z);
                }
            }
            samples += (winding != 0) ? 1 : 0;
        }
    }
    float alpha = floor((float(samples) * 255.0 + 8.0) / 16.0) / 255.0;
    return float4(snap3(value.color * alpha * 255.0) / 255.0, alpha);
}

float4 fragmentImage(Vertex value) : SV_Target {
    uint width;
    uint height;
    source.GetDimensions(width, height);
    float2 size = float2(width, height);
    float2 pt = clamp((value.position.xy + origin - first.xy) / first.zw * size - 0.5, float2(0.0, 0.0), size - 1.0);
    int2 p0 = int2(floor(pt));
    int2 p1 = min(p0 + 1, int2(width - 1, height - 1));
    float2 fracpt = frac(pt);
    float4 top = lerp(source.Load(int3(p0, 0)), source.Load(int3(p1.x, p0.y, 0)), fracpt.x);
    float4 bottom = lerp(source.Load(int3(p0.x, p1.y, 0)), source.Load(int3(p1, 0)), fracpt.x);
    uint count = 0;
    float2 pixel = floor(value.position.xy) + origin;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 s = pixel + (float2(column, row) + 0.5) / 4.0;
            count += (s.x >= second.x && s.y >= second.y && s.x < second.z && s.y < second.w) ? 1 : 0;
        }
    }
    return snap4(lerp(top, bottom, fracpt.y) * (float(count) / 16.0) * 255.0) / 255.0;
}

Vertex vertexCoverage(uint index : SV_VertexID, uint instance : SV_InstanceID) {
    uint4 strip = verts.Load4(instance * 16);
    float left = (float(strip.x) - first.z) / first.x * 2.0 - 1.0;
    float right = (float(strip.x + strip.z) - first.z) / first.x * 2.0 - 1.0;
    float top = 1.0 - (float(strip.y) - first.w) / first.y * 2.0;
    float bottom = 1.0 - (float(strip.y + 1) - first.w) / first.y * 2.0;
    float2 corners[6] = {
        float2(left, top),
        float2(left, bottom),
        float2(right, top),
        float2(right, top),
        float2(left, bottom),
        float2(right, bottom)
    };
    Vertex result;
    result.position = float4(corners[index], 0.0, 1.0);
    result.color = second.xyz;
    result.path = instance;
    return result;
}

float4 fragmentCoverage(Vertex value) : SV_Target {
    uint4 strip = data.Load4(value.path * 16);
    uint samples = 16;
    if (strip.w != 0) {
        int dx = int(floor(value.position.x) + first.z) - int(strip.x);
        if (dx < 0 || uint(dx) >= strip.z) {
            return float4(0.0, 0.0, 0.0, 0.0);
        }
        uint address = strip.w - 1 + uint(dx);
        samples = (data.Load((address / 4) * 4) >> ((address % 4) * 8)) & 255;
    }
    float alpha = float((samples * 255 + 8) / 16) / 255.0;
    return float4(snap3(value.color * alpha * 255.0) / 255.0, alpha);
}
