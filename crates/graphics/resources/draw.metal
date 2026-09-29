#include <metal_stdlib>
using namespace metal;

struct Vertex {
    float4 position [[position]];
    float3 color;
    uint path [[flat]];
};

vertex Vertex vertexMain(uint index [[vertex_id]], device const float* data [[buffer(0)]]) {
    uint offset = index * 5;
    return Vertex{float4(data[offset], data[offset + 1], 0.0, 1.0),
                  float3(data[offset + 2], data[offset + 3], data[offset + 4]), 0};
}

vertex Vertex vertexPath(uint index [[vertex_id]], uint instance [[instance_id]], device const float* data [[buffer(0)]]) {
    uint offset = instance * 8;
    float2 corners[6] = {
        float2(data[offset], data[offset + 1]),
        float2(data[offset], data[offset + 3]),
        float2(data[offset + 2], data[offset + 1]),
        float2(data[offset + 2], data[offset + 1]),
        float2(data[offset], data[offset + 3]),
        float2(data[offset + 2], data[offset + 3]),
    };
    return Vertex{float4(corners[index], 0.0, 1.0), float3(data[offset + 4], data[offset + 5], data[offset + 6]), instance};
}

fragment float4 fragmentMain(Vertex value [[stage_in]]) {
    return float4(value.color, 1.0);
}

fragment float4 fragmentComposite(Vertex value [[stage_in]], texture2d<float, access::read> source [[texture(0)]], constant float2& offset [[buffer(1)]]) {
    return source.read(uint2(value.position.xy + offset)) * value.color.r;
}

fragment float4 fragmentPath(Vertex value [[stage_in]], device const uint* data [[buffer(0)]], constant float2& origin [[buffer(3)]]) {
    uint offset = value.path * 8;
    device const float4* edges = reinterpret_cast<device const float4*>(data + data[offset]);
    device const uint* bands = data + data[offset + 1];
    device const float* settings = reinterpret_cast<device const float*>(data + offset + 4);
    uint band = (uint(floor(value.position.y) + origin.y) - data[offset + 2]) / data[offset + 3];
    uint begin = bands[band];
    uint end = bands[band + 1];
    uint samples = 0;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 point = (floor(value.position.xy) + origin) + (float2(column, row) + 0.5) / 4.0;
            if (point.x < settings[0] || point.y < settings[1] || point.x >= settings[2] || point.y >= settings[3]) continue;
            int winding = 0;
            for (uint index = begin; index < end; ++index) {
                float4 edge = edges[bands[index]];
                if ((edge.y <= point.y && edge.w > point.y) || (edge.w <= point.y && edge.y > point.y)) {
                    float cross = edge.x + (point.y - edge.y) / (edge.w - edge.y) * (edge.z - edge.x);
                    if (cross > point.x) winding += edge.w > edge.y ? 1 : -1;
                }
            }
            samples += winding != 0;
        }
    }
    float alpha = floor((float(samples) * 255.0 + 8.0) / 16.0) / 255.0;
    return float4(round(value.color * alpha * 255.0) / 255.0, alpha);
}

fragment float4 fragmentCurve(Vertex value [[stage_in]], device const uint* data [[buffer(0)]], constant float2& origin [[buffer(3)]]) {
    uint offset = value.path * 8;
    device const float4* curves = reinterpret_cast<device const float4*>(data + data[offset]);
    device const uint* bands = data + data[offset + 1];
    device const float* settings = reinterpret_cast<device const float*>(data + offset + 4);
    uint band = (uint(floor(value.position.y) + origin.y) - data[offset + 2]) / data[offset + 3];
    uint begin = bands[band];
    uint end = bands[band + 1];
    uint samples = 0;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 point = (floor(value.position.xy) + origin) + (float2(column, row) + 0.5) / 4.0;
            if (point.x < settings[0] || point.y < settings[1] || point.x >= settings[2] || point.y >= settings[3]) continue;
            int winding = 0;
            for (uint index = begin; index < end; ++index) {
                float4 first = curves[bands[index] * 2];
                float4 last = curves[bands[index] * 2 + 1];
                if (point.y < first.y || point.y >= last.y) continue;
                float dy = point.y - first.y;
                float cross;
                if (last.w == 0.0) {
                    cross = first.x + dy / (last.y - first.y) * (last.x - first.x);
                } else {
                    float b = first.w - first.y;
                    float a = (last.y - first.w) - b;
                    float divisor = b + sqrt(max(0.0, b * b + a * dy));
                    float t = dy == 0.0 ? 0.0 : clamp(dy / divisor, 0.0, 1.0);
                    float left = first.x + t * (first.z - first.x);
                    float right = first.z + t * (last.x - first.z);
                    cross = left + t * (right - left);
                }
                if (cross > point.x) winding += int(last.z);
            }
            samples += winding != 0;
        }
    }
    float alpha = floor((float(samples) * 255.0 + 8.0) / 16.0) / 255.0;
    return float4(round(value.color * alpha * 255.0) / 255.0, alpha);
}

fragment float4 fragmentImage(Vertex value [[stage_in]], texture2d<float, access::read> source [[texture(0)]], constant float* settings [[buffer(1)]], constant float2& origin [[buffer(3)]]) {
    float2 size = float2(source.get_width(), source.get_height());
    float2 point = clamp((value.position.xy + origin - float2(settings[0], settings[1])) / float2(settings[2], settings[3]) * size - 0.5, float2(0.0), size - 1.0);
    uint2 first = uint2(floor(point));
    uint2 last = min(first + 1, uint2(size) - 1);
    float2 fraction = fract(point);
    float4 top = mix(source.read(first), source.read(uint2(last.x, first.y)), fraction.x);
    float4 bottom = mix(source.read(uint2(first.x, last.y)), source.read(last), fraction.x);
    uint count = 0;
    for (uint row = 0; row < 4; ++row) {
        for (uint column = 0; column < 4; ++column) {
            float2 sample = (floor(value.position.xy) + origin) + (float2(column, row) + 0.5) / 4.0;
            count += sample.x >= settings[4] && sample.y >= settings[5] && sample.x < settings[6] && sample.y < settings[7];
        }
    }
    return round(mix(top, bottom, fraction.y) * (float(count) / 16.0) * 255.0) / 255.0;
}

vertex Vertex vertexCoverage(uint index [[vertex_id]], uint instance [[instance_id]], device const uint4* strips [[buffer(0)]], constant float* settings [[buffer(1)]]) {
    uint4 strip = strips[instance];
    float left = (float(strip.x) - settings[2]) / settings[0] * 2.0 - 1.0;
    float right = (float(strip.x + strip.z) - settings[2]) / settings[0] * 2.0 - 1.0;
    float top = 1.0 - (float(strip.y) - settings[3]) / settings[1] * 2.0;
    float bottom = 1.0 - (float(strip.y + 1) - settings[3]) / settings[1] * 2.0;
    float2 corners[6] = { float2(left,top), float2(left,bottom), float2(right,top), float2(right,top), float2(left,bottom), float2(right,bottom) };
    return Vertex{float4(corners[index],0.0,1.0),float3(settings[4],settings[5],settings[6]),instance};
}
fragment float4 fragmentCoverage(Vertex value [[stage_in]], device const uint* data [[buffer(0)]], constant float* settings [[buffer(1)]]) {
    uint offset = data[value.path * 4 + 3];
    uint samples = 16;
    if (offset != 0) {
        uint address = offset - 1 + uint(floor(value.position.x) + settings[2]) - data[value.path * 4];
        samples = (data[address / 4] >> ((address % 4) * 8)) & 255;
    }
    float alpha = float((samples * 255 + 8) / 16) / 255.0;
    return float4(round(value.color * alpha * 255.0) / 255.0,alpha);
}
