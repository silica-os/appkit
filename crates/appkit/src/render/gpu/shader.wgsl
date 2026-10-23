// One shader for every draw of the GPU canvas: the vertex stage maps device pixels to clip
// space; the fragment stage evaluates a paint (solid, gradients, image, blurred rounded rect,
// backdrop with glass) and multiplies it by the clip mask.

struct Paint {
    inv_a: vec4<f32>,   // device -> paint space: a b c d
    inv_b: vec4<f32>,   // e f, target width, target height
    color: vec4<f32>,   // premultiplied colour (solid) | opacity in x (textures)
    g0: vec4<f32>,      // gradient/image/rrect geometry
    g1: vec4<f32>,      // kind, stop count, uses mask, extra
    g2: vec4<f32>,      // rrect: sigma, radius, invert | backdrop: origin.xy, size.xy
    g3: vec4<f32>,      // glass: rect (layer coordinates)
    g4: vec4<f32>,      // glass: distortion, depth, chromatic, magnification
    g5: vec4<f32>,      // glass: radius, smoothing, enabled, 0
    m0: vec4<f32>,      // saturation/brightness matrix rows
    m1: vec4<f32>,
    m2: vec4<f32>,
    fwd_a: vec4<f32>,   // layer -> device: a b c d
    fwd_b: vec4<f32>,   // e f
    h0: vec4<f32>,      // clip mask: origin.xy, size.zw (device pixels)
    h1: vec4<f32>,      // the target's origin (device pixels)
    stops_t: array<vec4<f32>, 2>,
    stops_c: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var<uniform> P: Paint;
@group(0) @binding(1) var t_img: texture_2d<f32>;
@group(0) @binding(2) var t_mask: texture_2d<f32>;
@group(0) @binding(3) var smp: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs(@location(0) p: vec2<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>((p.x - P.h1.x) / P.inv_b.z * 2.0 - 1.0, 1.0 - (p.y - P.h1.y) / P.inv_b.w * 2.0, 0.0, 1.0);
    return o;
}

@vertex
fn vs_stencil(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>((p.x - P.h1.x) / P.inv_b.z * 2.0 - 1.0, 1.0 - (p.y - P.h1.y) / P.inv_b.w * 2.0, 0.0, 1.0);
}

fn to_paint(d: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        P.inv_a.x * d.x + P.inv_a.z * d.y + P.inv_b.x,
        P.inv_a.y * d.x + P.inv_a.w * d.y + P.inv_b.y,
    );
}

fn gradient(t_in: f32) -> vec4<f32> {
    let n = i32(P.g1.y);
    if (n <= 0) { return vec4<f32>(0.0); }
    let t = clamp(t_in, 0.0, 1.0);
    var prev_t = P.stops_t[0].x;
    var prev_c = P.stops_c[0];
    if (t <= prev_t) { return vec4<f32>(prev_c.rgb * prev_c.a, prev_c.a); }
    for (var i = 1; i < n; i = i + 1) {
        let st = P.stops_t[i / 4][i % 4];
        let sc = P.stops_c[i];
        if (t <= st) {
            let k = (t - prev_t) / max(st - prev_t, 1e-6);
            let c = mix(prev_c, sc, k);
            return vec4<f32>(c.rgb * c.a, c.a);
        }
        prev_t = st;
        prev_c = sc;
    }
    return vec4<f32>(prev_c.rgb * prev_c.a, prev_c.a);
}

// Abramowitz-Stegun erf.
fn erf(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * exp(-a * a);
    return s * y;
}

// The rounded rectangle blurred by a Gaussian (Evan Wallace's closed form: the rectangle's
// coverage is a product of erf terms; the corner is a sum over rows).
fn rrect_blur(d: vec2<f32>) -> f32 {
    let r = P.g0;                 // x y w h (device)
    let sigma = max(P.g2.x, 0.01);
    let radius = min(P.g2.y, min(r.z, r.w) * 0.5);
    let c = vec2<f32>(r.x + r.z * 0.5, r.y + r.w * 0.5);
    let half = vec2<f32>(r.z * 0.5, r.w * 0.5);
    let p = d - c;
    let s2 = sigma * 1.41421356;
    var acc = 0.0;
    let steps = 8;
    for (var i = 0; i < steps; i = i + 1) {
        // Rows over the blur's reach in y, weighted by the Gaussian.
        let k = (f32(i) + 0.5) / f32(steps);
        let o = -3.0 * sigma + 6.0 * sigma * k;
        let y = p.y + o;
        let w = exp(-o * o / (2.0 * sigma * sigma));
        // The rounded rect's horizontal extent at row y.
        let dy = abs(y) - (half.y - radius);
        var ext = half.x;
        if (dy > 0.0) {
            ext = half.x - radius + sqrt(max(radius * radius - dy * dy, 0.0));
        }
        var cov = 0.0;
        if (abs(y) <= half.y) {
            cov = 0.5 * (erf((p.x + ext) / s2) - erf((p.x - ext) / s2));
        }
        acc = acc + w * cov;
    }
    // The weights sum to the Gaussian's mass over the sampled reach.
    var wsum = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let k = (f32(i) + 0.5) / f32(steps);
        let y = -3.0 * sigma + 6.0 * sigma * k;
        wsum = wsum + exp(-y * y / (2.0 * sigma * sigma));
    }
    return clamp(acc / wsum, 0.0, 1.0);
}

// Ossido's signed distance to a continuous rounded rect (negative inside).
fn continuous_distance(rect: vec4<f32>, radius: f32, smoothing: f32, p: vec2<f32>) -> f32 {
    let c = vec2<f32>(rect.x + rect.z * 0.5, rect.y + rect.w * 0.5);
    let l = abs(p - c);
    let a = min(radius * (1.0 + smoothing), min(rect.z, rect.w) * 0.5);
    let dx = l.x - (rect.z * 0.5 - a);
    let dy = l.y - (rect.w * 0.5 - a);
    if (a > 0.0 && dx > 0.0 && dy > 0.0) {
        let n = 2.0 + 2.55 * clamp(smoothing, 0.0, 1.0);
        let qx = dx / a;
        let qy = dy / a;
        let f = pow(pow(qx, n) + pow(qy, n), 1.0 / n);
        let g = sqrt(pow(qx, 2.0 * n - 2.0) + pow(qy, 2.0 * n - 2.0)) / pow(f, n - 1.0);
        return (f - 1.0) * a / max(g, 0.001);
    }
    let ox = max(dx, 0.0);
    let oy = max(dy, 0.0);
    return min(max(dx, dy), 0.0) + sqrt(ox * ox + oy * oy) - a;
}

fn layer_to_device(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        P.fwd_a.x * p.x + P.fwd_a.z * p.y + P.fwd_b.x,
        P.fwd_a.y * p.x + P.fwd_a.w * p.y + P.fwd_b.y,
    );
}

fn fetch_dev(dev: vec2<f32>) -> vec4<f32> {
    let uv = (dev - P.g2.xy) / P.g2.zw;
    return textureSampleLevel(t_img, smp, uv, 0.0);
}

fn backdrop(frag: vec2<f32>) -> vec4<f32> {
    var s = fetch_dev(frag);
    if (P.g5.z > 0.5) {
        let rect = P.g3;
        let p = to_paint(frag);
        let c = vec2<f32>(rect.x + rect.z * 0.5, rect.y + rect.w * 0.5);
        let mag = max(P.g4.w, 0.01);
        let q = c + (p - c) / mag;
        let band = P.g4.y * min(rect.z, rect.w) * 0.5;
        let radius = min(rect.z, rect.w) * 0.5;
        let sm = P.g5.y;
        let d = -continuous_distance(rect, radius, sm, p);
        let magnify = abs(P.g4.w - 1.0) > 1e-3;
        if ((band <= 0.0 || d >= band || d < 0.0)) {
            if (magnify) { s = fetch_dev(layer_to_device(q)); }
        } else {
            let e = 0.5;
            var n = vec2<f32>(
                continuous_distance(rect, radius, sm, p + vec2<f32>(e, 0.0)) - continuous_distance(rect, radius, sm, p - vec2<f32>(e, 0.0)),
                continuous_distance(rect, radius, sm, p + vec2<f32>(0.0, e)) - continuous_distance(rect, radius, sm, p - vec2<f32>(0.0, e)),
            );
            let ln = length(n);
            if (ln > 1e-4) { n = n / ln; } else { n = vec2<f32>(0.0); }
            let t = d / band;
            let shift = max(P.g4.x, 0.0) * band * (1.0 - t) * (1.0 - t);
            let sp = q - n * shift;
            let split = max(P.g4.z, 0.0) * band * (1.0 - t);
            let mid = fetch_dev(layer_to_device(sp));
            s = mid;
            if (split > 0.0) {
                let rr = fetch_dev(layer_to_device(sp + n * split)).x;
                let bb = fetch_dev(layer_to_device(sp - n * split)).z;
                s = vec4<f32>(min(rr, mid.w), mid.y, min(bb, mid.w), mid.w);
            }
        }
    }
    // Saturation and brightness on the unpremultiplied colour.
    let a = max(s.w, 1e-6);
    let rgb = s.rgb / a;
    let o = vec3<f32>(dot(P.m0.xyz, rgb), dot(P.m1.xyz, rgb), dot(P.m2.xyz, rgb));
    let oc = clamp(o, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(oc * s.w, s.w);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let kind = i32(P.g1.x);
    let d = frag.xy + P.h1.xy;
    var out = vec4<f32>(0.0);
    if (kind == 0) {
        out = P.color;
    } else if (kind == 1) {
        let p = to_paint(d);
        let a = P.g0.xy;
        let b = P.g0.zw;
        let ab = b - a;
        out = gradient(dot(p - a, ab) / max(dot(ab, ab), 1e-9)) * P.color.x;
    } else if (kind == 2) {
        let p = to_paint(d);
        out = gradient(length(p - P.g0.xy) / max(P.g0.z, 1e-6)) * P.color.x;
    } else if (kind == 3) {
        let p = to_paint(d) - P.g0.xy;
        var t = (atan2(p.y, p.x) - P.g0.z) / 6.28318530718;
        t = t - floor(t);
        out = gradient(t) * P.color.x;
    } else if (kind == 4) {
        let p = to_paint(d);
        let uv = p / P.g0.xy;
        out = textureSampleLevel(t_img, smp, uv, 0.0) * P.color.x;
        if (P.g1.w > 1.5) {
            // A coverage texture (the glyph atlas, R8) tinted by the colour.
            out = P.color * textureSampleLevel(t_img, smp, uv, 0.0).x;
        } else if (P.g1.w > 0.5) {
            // A coverage image tinted by the colour.
            out = P.color * textureSampleLevel(t_img, smp, uv, 0.0).w;
        }
    } else if (kind == 5) {
        out = backdrop(d) * P.color.x;
    } else if (kind == 6) {
        var c = rrect_blur(d);
        if (P.g2.z > 0.5) { c = 1.0 - c; }
        out = P.color * c;
    }
    if (P.g1.z > 0.5) {
        let m = textureSampleLevel(t_mask, smp, (d - P.h0.xy) / P.h0.zw, 0.0).x;
        out = out * m;
    }
    return out;
}

@fragment
fn fs_stencil() {}

// --- Blur passes (full-screen over a target of the size in inv_b.zw) --------------------

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // g0.xy: the direction in texels, g0.z: sigma (texels), g0.w: tap radius.
    let size = vec2<f32>(textureDimensions(t_img));
    let sigma = max(P.g0.z, 0.01);
    let r = i32(P.g0.w);
    var acc = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -r; i <= r; i = i + 1) {
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        let uv = (frag.xy + P.g0.xy * f32(i)) / size;
        acc = acc + w * textureSampleLevel(t_img, smp, uv, 0.0);
        total = total + w;
    }
    return acc / total;
}

@fragment
fn fs_copy(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // Bilinear reduction/copy of the texture into a target of g0.xy texels.
    let uv = frag.xy / P.g0.xy;
    return textureSampleLevel(t_img, smp, uv, 0.0);
}
