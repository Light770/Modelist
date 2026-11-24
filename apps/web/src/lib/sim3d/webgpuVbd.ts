/**
 * Lazy WebGPU WGSL Compute Engine for Graph-Colored Vertex Block Descent (VBD).
 *
 * Strictly isolated from 2D drafting: never initialized unless the 3D Fit Room opens,
 * and releases all GPU storage buffers immediately on dispose().
 */

interface GpuBufferLike {
  destroy(): void;
}

interface GpuDeviceLike {
  createBuffer(desc: { size: number; usage: number }): GpuBufferLike;
  createShaderModule(desc: { code: string }): unknown;
  createComputePipeline(desc: unknown): unknown;
  queue: {
    writeBuffer(buf: GpuBufferLike, offset: number, data: ArrayBufferView): void;
  };
  destroy?(): void;
}

interface GpuAdapterLike {
  requestDevice(): Promise<GpuDeviceLike>;
}

interface NavigatorGpuLike {
  requestAdapter(): Promise<GpuAdapterLike | null>;
}

const GPU_BUFFER_USAGE_STORAGE = 0x0080;
const GPU_BUFFER_USAGE_COPY_DST = 0x0008;
const GPU_BUFFER_USAGE_COPY_SRC = 0x0004;

export const PREDICT_INERTIA_WGSL = /* wgsl */ `
struct Params {
  vertex_count: u32,
  dt: f32,
  gravity_y: f32,
  damping: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> pos: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> prev: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> y_target: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read> inv_mass: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= params.vertex_count) { return; }
  let x = pos[i].xyz;
  if (inv_mass[i] <= 0.0) {
    y_target[i] = vec4<f32>(x, 0.0);
    prev[i] = vec4<f32>(x, 0.0);
    return;
  }
  let v_dt = (x - prev[i].xyz) * exp(-params.damping * params.dt);
  let y = x + v_dt + vec3<f32>(0.0, params.gravity_y * params.dt * params.dt, 0.0);
  y_target[i] = vec4<f32>(y, 0.0);
  prev[i] = vec4<f32>(x, 0.0);
  pos[i] = vec4<f32>(y, 0.0);
}
`;

export const VBD_COLOR_STEP_WGSL = /* wgsl */ `
struct StepParams {
  color_start: u32,
  color_end: u32,
  inv_dt2: f32,
  _pad: u32,
};

@group(0) @binding(0) var<uniform> step_p: StepParams;
@group(0) @binding(1) var<storage, read_write> pos: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> y_target: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> inv_mass: array<f32>;
@group(0) @binding(4) var<storage, read> color_verts: array<u32>;
@group(0) @binding(5) var<storage, read> vert_tri_offsets: array<u32>;
@group(0) @binding(6) var<storage, read> vert_tri_entries: array<u32>;
@group(0) @binding(7) var<storage, read> tri_verts: array<vec4<u32>>;
@group(0) @binding(8) var<storage, read> tri_cuv: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let idx = step_p.color_start + gid.x;
  if (idx >= step_p.color_end) { return; }
  let i = color_verts[idx];
  let im = inv_mass[i];
  if (im <= 0.0) { return; }
  let xi = pos[i].xyz;
  let m_dt2 = (1.0 / im) * step_p.inv_dt2;
  var grad = (xi - y_target[i].xyz) * m_dt2;
  var h_diag = vec3<f32>(m_dt2);
  let ts = vert_tri_offsets[i];
  let te = vert_tri_offsets[i + 1u];
  for (var e = ts; e < te; e = e + 1u) {
    let entry = vert_tri_entries[e];
    let ti = entry >> 2u;
    let slot = entry & 3u;
    let tv = tri_verts[ti];
    let c0 = tri_cuv[ti * 3u + 0u];
    let c1 = tri_cuv[ti * 3u + 1u];
    let c2 = tri_cuv[ti * 3u + 2u];
    let cu_i = c0[slot];
    let cv_i = c1[slot];
    let area = c0.w;
    let fu = pos[tv.x].xyz * c0.x + pos[tv.y].xyz * c0.y + pos[tv.z].xyz * c0.z;
    let lu = max(length(fu), 1e-6);
    let nu = fu / lu;
    let ku = area * c1.w;
    grad += nu * (ku * (lu - c2.z) * cu_i);
    h_diag += vec3<f32>(ku * cu_i * cu_i) * (nu * nu);
    let fv = pos[tv.x].xyz * c1.x + pos[tv.y].xyz * c1.y + pos[tv.z].xyz * c1.z;
    let lv = max(length(fv), 1e-6);
    let nv = fv / lv;
    let kv = area * c2.x;
    grad += nv * (kv * (lv - c2.w) * cv_i);
    h_diag += vec3<f32>(kv * cv_i * cv_i) * (nv * nv);
  }
  let dx = clamp(-grad / max(h_diag, vec3<f32>(1e-5)), vec3<f32>(-0.02), vec3<f32>(0.02));
  pos[i] = vec4<f32>(xi + dx, 0.0);
}
`;

export class WebGpuVbdEngine {
  private device: GpuDeviceLike;
  private buffers: GpuBufferLike[] = [];

  private constructor(device: GpuDeviceLike) {
    this.device = device;
  }

  static async create(
    colorVerts: Uint32Array,
    vertTriOffsets: Uint32Array,
    vertTriEntries: Uint32Array,
    triVerts: Uint32Array,
    triCuv: Float32Array,
    invMass: Float32Array,
  ): Promise<WebGpuVbdEngine | null> {
    if (typeof navigator === 'undefined') return null;
    const gpu = (navigator as unknown as { gpu?: NavigatorGpuLike }).gpu;
    if (!gpu) return null;
    try {
      const adapter = await gpu.requestAdapter();
      if (!adapter) return null;
      const device = await adapter.requestDevice();
      const engine = new WebGpuVbdEngine(device);
      engine.uploadStatic(colorVerts, vertTriOffsets, vertTriEntries, triVerts, triCuv, invMass);
      return engine;
    } catch {
      return null;
    }
  }

  private uploadStatic(
    colorVerts: Uint32Array,
    vertTriOffsets: Uint32Array,
    vertTriEntries: Uint32Array,
    triVerts: Uint32Array,
    triCuv: Float32Array,
    invMass: Float32Array,
  ) {
    this.clearBuffers();
    this.device.createShaderModule({ code: PREDICT_INERTIA_WGSL });
    this.device.createShaderModule({ code: VBD_COLOR_STEP_WGSL });
    const alloc = (arr: ArrayBufferView) => {
      const size = Math.max(16, Math.ceil(arr.byteLength / 4) * 4);
      const buf = this.device.createBuffer({
        size,
        usage: GPU_BUFFER_USAGE_STORAGE | GPU_BUFFER_USAGE_COPY_DST | GPU_BUFFER_USAGE_COPY_SRC,
      });
      this.device.queue.writeBuffer(buf, 0, arr);
      this.buffers.push(buf);
    };
    alloc(colorVerts);
    alloc(vertTriOffsets);
    alloc(vertTriEntries);
    alloc(triVerts);
    alloc(triCuv);
    alloc(invMass);
  }

  private clearBuffers() {
    for (const b of this.buffers) {
      b.destroy();
    }
    this.buffers = [];
  }

  dispose() {
    this.clearBuffers();
    this.device.destroy?.();
  }
}

