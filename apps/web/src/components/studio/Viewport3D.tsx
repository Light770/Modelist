import React, { useEffect, useRef, useState } from 'react';
import { Focus, Grid3x3, Hand, Layers, Pause, PersonStanding, Play, RotateCcw, Ruler, Wind, X } from 'lucide-react';
import { getSim3dVersion, kernel } from '../../lib/kernelClient';
import { WebGpuVbdEngine } from '../../lib/sim3d/webgpuVbd';
import {
  CAMERA_FOV_RAD,
  type OrbitCamera3D,
  applyFreeMove,
  applyOrbitDelta,
  applyPanDelta,
  applyZoomFactor,
  cameraBasis,
  cameraClipPlanes,
  makeViewMatrix,
  unprojectCameraRay,
} from '../../lib/sim3d/camera3d';
import { useT, type MessageKey } from '../../lib/i18n';
import type { Camera3DPreset, FabricFinish, Sim3dStats } from '../../lib/types';

const FINISH_CODE: Record<FabricFinish, number> = {
  plain: 0,
  twill: 1,
  satin: 2,
  jersey: 3,
  rib: 4,
  sheer: 5,
  terry: 6,
};
import { useStudioStore } from '../../store/studioStore';
import { wheelZoomFactor } from '../../lib/cad/zoom';

const VS_SRC = `#version 300 es
precision highp float;
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;
layout(location = 2) in float a_metric;
layout(location = 3) in vec3 a_color;
layout(location = 4) in float a_seam;

uniform mat4 u_mvp;
out vec3 v_norm;
out vec3 v_pos;
out float v_metric;
out vec3 v_color;
out float v_seam;

void main() {
  v_norm = a_norm;
  v_pos = a_pos;
  v_metric = a_metric;
  v_color = a_color;
  v_seam = a_seam;
  gl_Position = u_mvp * vec4(a_pos, 1.0);
}
`;

const FS_SRC = `#version 300 es
precision highp float;
in vec3 v_norm;
in vec3 v_pos;
in float v_metric;
in vec3 v_color;
in float v_seam;

uniform int u_mode; // 0=avatar, 1=cloth_material, 2=strain, 3=tension, 4=clearance, 5=xray, 6=floor
uniform int u_finish; // 0 plain, 1 twill, 2 satin, 3 jersey, 4 rib, 5 sheer, 6 terry
uniform vec3 u_baseColor;
uniform vec3 u_eyePos;
uniform float u_isDark;
uniform vec4 u_tapeY; // (bust_y, waist_y, hip_y, crotch_y)
uniform float u_neckY;
uniform float u_showTape;
out vec4 outColor;

vec3 heatRamp(float t) {
  t = clamp(t, 0.0, 1.0);
  if (t < 0.35) {
    return mix(vec3(0.10, 0.48, 0.90), vec3(0.08, 0.74, 0.50), t / 0.35);
  } else if (t < 0.70) {
    return mix(vec3(0.08, 0.74, 0.50), vec3(0.95, 0.68, 0.12), (t - 0.35) / 0.35);
  } else {
    return mix(vec3(0.95, 0.68, 0.12), vec3(0.88, 0.16, 0.18), (t - 0.70) / 0.30);
  }
}

void main() {
  // Studio Floor Plane with Architectural 20cm Grid + Soft Radial Contact Shadow
  if (u_mode == 6) {
    float r = length(v_pos.xz);
    float fade = 1.0 - smoothstep(2.0, 7.5, r);
    if (fade < 0.01) discard;

    vec2 g = abs(fract(v_pos.xz * 5.0 - 0.5) - 0.5) / max(fwidth(v_pos.xz * 5.0), vec2(0.001));
    float line = 1.0 - min(min(g.x, g.y), 1.0);
    vec2 gmaj = abs(fract(v_pos.xz * 1.0 - 0.5) - 0.5) / max(fwidth(v_pos.xz * 1.0), vec2(0.001));
    float majLine = 1.0 - min(min(gmaj.x, gmaj.y), 1.0);

    // Soft studio ambient-occlusion contact shadow under the mannequin feet
    float shadow = 0.24 * exp(-r * r * 14.0);

    vec3 floorBase = mix(vec3(0.92, 0.91, 0.89), vec3(0.12, 0.13, 0.16), u_isDark);
    vec3 gridCol = mix(vec3(0.76, 0.75, 0.72), vec3(0.24, 0.26, 0.32), u_isDark);
    vec3 majCol = mix(vec3(0.62, 0.61, 0.59), vec3(0.32, 0.35, 0.42), u_isDark);
    vec3 col = mix(floorBase, gridCol, line * 0.45);
    col = mix(col, majCol, majLine * 0.65);
    col *= (1.0 - shadow);
    outColor = vec4(col, fade * 0.92);
    return;
  }

  vec3 n = normalize(v_norm);
  bool backFace = !gl_FrontFacing;
  if (backFace) n = -n;
  vec3 vDir = normalize(u_eyePos - v_pos);

  // Studio 3-point lighting (warm key + cool fill + rim backlight)
  vec3 lKey = normalize(vec3(0.55, 0.82, 0.72));
  vec3 lFill = normalize(vec3(-0.75, 0.35, 0.45));
  vec3 lRim = normalize(vec3(-0.25, 0.55, -0.85));

  float key = max(dot(n, lKey), 0.0);
  float fill = max(dot(n, lFill), 0.0);
  float rim = max(dot(n, lRim), 0.0);
  float fresnel = pow(1.0 - max(dot(n, vDir), 0.0), 3.0);
  float hemi = 0.30 + 0.16 * n.y;

  if (u_mode == 0) {
    // Sculpted atelier porcelain-clay dress-form mannequin + calibrated tailor tape lines
    float wrap = max((dot(n, lKey) + 0.30) / 1.30, 0.0);
    float spec = pow(max(dot(reflect(-lKey, n), vDir), 0.0), 22.0) * 0.10;
    float ao = 0.88 + 0.12 * clamp(0.5 + 0.5 * n.y, 0.0, 1.0);
    float light = clamp(hemi + wrap * 0.56 + fill * 0.22 + rim * 0.14, 0.26, 1.10) * ao;
    vec3 col = u_baseColor * light + vec3(spec) + vec3(0.055) * fresnel;

    if (u_showTape > 0.5 && v_metric > 0.55 && v_pos.y > u_tapeY.w + 0.015 && v_pos.y < u_neckY - 0.015) {
      float dyB = abs(v_pos.y - u_tapeY.x) / max(fwidth(v_pos.y), 0.0005);
      float dyW = abs(v_pos.y - u_tapeY.y) / max(fwidth(v_pos.y), 0.0005);
      float dyH = abs(v_pos.y - u_tapeY.z) / max(fwidth(v_pos.y), 0.0005);
      float dxC = abs(v_pos.x) / max(fwidth(v_pos.x), 0.0005);
      float hLine = 1.0 - smoothstep(0.9, 2.2, min(min(dyB, dyW), dyH));
      float vLine = 1.0 - smoothstep(0.8, 1.9, dxC);
      float tape = max(hLine * 0.55, vLine * 0.34);
      vec3 tapeCol = mix(vec3(0.24, 0.25, 0.28), vec3(0.75, 0.78, 0.84), u_isDark);
      col = mix(col, tapeCol, tape);
    }

    outColor = vec4(col, 1.0);
    return;
  }

  if (u_mode == 1) {
    // Per-piece fabric colour with a procedural surface finish (look only, no physics).
    vec3 base = v_color;
    float tex = 1.0;
    float sheenAmt = 0.18;
    float specPow = 0.0;
    float alpha = 1.0;
    if (u_finish == 1) {            // twill: diagonal wales
      tex = 0.94 + 0.06 * sin((v_pos.x + v_pos.y) * 900.0);
    } else if (u_finish == 2) {     // satin: smooth with a strong sheen
      tex = 1.0; sheenAmt = 0.45; specPow = 28.0;
    } else if (u_finish == 3) {     // jersey: fine vertical knit
      tex = 0.95 + 0.05 * abs(sin(v_pos.x * 1400.0 + v_pos.z * 1400.0));
    } else if (u_finish == 4) {     // rib: pronounced vertical ribs
      tex = 0.86 + 0.14 * smoothstep(-0.2, 0.9, sin(v_pos.x * 420.0 + v_pos.z * 420.0));
    } else if (u_finish == 5) {     // sheer: light and translucent
      base = mix(base, vec3(1.0), 0.18); alpha = 0.72; sheenAmt = 0.3;
    } else if (u_finish == 6) {     // terry: matte, slightly mottled
      tex = 0.93 + 0.07 * fract(sin(dot(floor(v_pos * 700.0), vec3(12.9898, 78.233, 37.719))) * 43758.5453);
      sheenAmt = 0.06;
    } else {                        // plain weave
      tex = 0.97 + 0.03 * sin(v_pos.x * 260.0 + v_pos.y * 180.0) * cos(v_pos.y * 260.0 - v_pos.z * 180.0);
    }
    vec3 fabricCol = base * tex;
    if (v_seam > 0.25) {
      // Pressed seamline groove shading + subtle hem-fold & interior serge hint
      float groove = smoothstep(0.25, 1.0, v_seam) * 0.08;
      fabricCol *= (1.0 - groove);
      if (!backFace && v_seam > 2.5 && v_seam < 3.5) {
        fabricCol *= 0.93;
      } else if (backFace && v_seam > 1.5 && v_seam < 2.5) {
        // Surjettage (overlock serging) loops on interior back-faces
        float loopWave = abs(sin((v_pos.x - v_pos.y + v_pos.z) * 320.0));
        vec3 sergeCol = mix(base * 1.12, vec3(0.86, 0.88, 0.92), 0.35);
        fabricCol = mix(fabricCol, sergeCol, smoothstep(0.65, 0.96, loopWave) * 0.22);
      }
    }
    if (backFace) {
      fabricCol *= vec3(0.74, 0.72, 0.70); // darker inside, like CLO3D
    }
    float wrapDiff = max((dot(n, lKey) + 0.25) / 1.25, 0.0);
    float light = clamp(hemi * 0.92 + wrapDiff * 0.62 + fill * 0.24 + rim * 0.16, 0.22, 1.12);
    vec3 sheen = mix(base, vec3(1.0), 0.35) * fresnel * sheenAmt;
    float spec = specPow > 0.0 ? pow(max(dot(reflect(-lKey, n), vDir), 0.0), specPow) * 0.35 : 0.0;
    outColor = vec4(fabricCol * light + sheen + vec3(spec), alpha);
    return;
  }

  float shade = clamp(hemi + key * 0.55 + fill * 0.20, 0.35, 1.08);
  if (u_mode == 2) {
    vec3 col = heatRamp(v_metric / 12.0) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  if (u_mode == 3) {
    vec3 col = heatRamp(v_metric / 160.0) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  if (u_mode == 4) {
    // Ease / Clearance (mm): <4mm red compression, 15-35mm green ideal, >65mm blue loose
    float t = 1.0 - clamp((v_metric - 2.0) / 68.0, 0.0, 1.0);
    vec3 col = heatRamp(t) * (0.42 + 0.65 * shade);
    outColor = vec4(col, 1.0);
    return;
  }
  // X-Ray translucent fit check
  outColor = vec4(vec3(0.22, 0.58, 0.96) * shade + vec3(0.15) * fresnel, 0.42);
}
`;

function hexToRgb01(hex: string): [number, number, number] {
  const clean = hex.replace('#', '');
  if (clean.length !== 6) return [0.22, 0.36, 0.55];
  const r = parseInt(clean.slice(0, 2), 16) / 255;
  const g = parseInt(clean.slice(2, 4), 16) / 255;
  const b = parseInt(clean.slice(4, 6), 16) / 255;
  return [
    Number.isFinite(r) ? r : 0.22,
    Number.isFinite(g) ? g : 0.36,
    Number.isFinite(b) ? b : 0.55,
  ];
}

function compileShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type)!;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(sh) || 'Shader compile failed');
  }
  return sh;
}

function makePerspective(fovRad: number, aspect: number, near: number, far: number): Float32Array {
  const f = 1 / Math.tan(fovRad / 2);
  const nf = 1 / (near - far);
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (far + near) * nf, -1,
    0, 0, 2 * far * near * nf, 0,
  ]);
}

/**
 * Right-handed OpenGL LookAt matrix with world up = (0, +1, 0).
 * Camera looks along -fz, with right = normalize(up x fz) and trueUp = fz x right.
 */
function mulMat4(a: Float32Array, b: Float32Array): Float32Array {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      out[c * 4 + r] =
        a[0 * 4 + r] * b[c * 4 + 0] +
        a[1 * 4 + r] * b[c * 4 + 1] +
        a[2 * 4 + r] * b[c * 4 + 2] +
        a[3 * 4 + r] * b[c * 4 + 3];
    }
  }
  return out;
}

function defaultFramingForTemplate(templateId?: string) {
  if (
    templateId === 'trouser_block' ||
    templateId === 'wide_pleat_trouser' ||
    templateId === 'barrel_trouser' ||
    templateId === 'jogger_pants'
  ) {
    return { targetY: 0.62, dist: 1.92 };
  }
  if (templateId === 'aline_skirt') {
    return { targetY: 0.80, dist: 1.78 };
  }
  return { targetY: 1.18, dist: 1.68 };
}

export const Viewport3D: React.FC<{ style?: React.CSSProperties }> = ({ style }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const snapshot = useStudioStore((s) => s.snapshot);
  const initialFrame = defaultFramingForTemplate(snapshot?.template_id);
  const orbitRef = useRef<OrbitCamera3D>({
    yaw: 0.36,
    pitch: 0.08,
    dist: initialFrame.dist,
    targetX: 0,
    targetY: initialFrame.targetY,
    targetZ: 0,
  });
  const navRef = useRef<{
    mode: 'none' | 'orbit' | 'pan';
    yawSign: number;
    lx: number;
    ly: number;
    pointers: Map<number, { x: number; y: number }>;
  }>({
    mode: 'none',
    yawSign: 1,
    lx: 0,
    ly: 0,
    pointers: new Map(),
  });
  const keysRef = useRef<Set<string>>(new Set());
  const playingRef = useRef(true);
  const lastVerRef = useRef(-1);

  const pose = useStudioStore((s) => s.sim3dPose);
  const bodyForm = useStudioStore((s) => s.sim3dBodyForm);
  const viewMode = useStudioStore((s) => s.sim3dViewMode);
  const showAvatar = useStudioStore((s) => s.show3dAvatar);
  const showFloor = useStudioStore((s) => s.show3dFloor);
  const showTape = useStudioStore((s) => s.show3dTapeLines);
  const cameraPreset = useStudioStore((s) => s.sim3dCameraPreset);
  const cameraSeq = useStudioStore((s) => s.sim3dCameraSeq);

  const toggle3dAvatar = useStudioStore((s) => s.toggle3dAvatar);
  const toggle3dFloor = useStudioStore((s) => s.toggle3dFloor);
  const toggle3dTapeLines = useStudioStore((s) => s.toggle3dTapeLines);
  const request3dCamera = useStudioStore((s) => s.request3dCamera);
  const playing = useStudioStore((s) => s.sim3dPlaying);
  const setPlaying = useStudioStore((s) => s.setSim3dPlaying);
  const requestSim3dReset = useStudioStore((s) => s.requestSim3dReset);
  const resetSeq = useStudioStore((s) => s.sim3dResetSeq);
  const setSim3dTelemetry = useStudioStore((s) => s.setSim3dTelemetry);

  const statsRef = useRef<Sim3dStats | null>(null);
  const [glError, setGlError] = useState<string | null>(null);
  const [grabMode, setGrabMode] = useState(false);
  const [meshSpacing, setMeshSpacing] = useState(0);
  const [selfCollide, setSelfCollide] = useState(true);
  const [windOn, setWindOn] = useState(false);
  const [pinCount, setPinCount] = useState(0);
  const [webgpuActive, setWebgpuActive] = useState(false);
  const grabRef = useRef<{
    planePoint: [number, number, number];
    planeNormal: [number, number, number];
  } | null>(null);
  const webgpuRef = useRef<WebGpuVbdEngine | null>(null);
  const t = useT();

  const publishStats = (next: Sim3dStats | null) => {
    statsRef.current = next;
    setPinCount(next?.pin_count ?? 0);
    setSim3dTelemetry(next);
  };

  useEffect(() => {
    playingRef.current = playing;
  }, [playing]);

  const initWebGpuEngine = () => {
    webgpuRef.current?.dispose();
    webgpuRef.current = null;
    const meta = kernel.getSim3dVbdGpuMeta();
    if (!meta) return;
    void WebGpuVbdEngine.create(
      kernel.getSim3dVbdGpuU32('color_verts'),
      kernel.getSim3dVbdGpuU32('vert_tri_offsets'),
      kernel.getSim3dVbdGpuU32('vert_tri_entries'),
      kernel.getSim3dVbdGpuU32('tri_verts'),
      kernel.getSim3dVbdGpuF32('tri_cuv'),
      kernel.getSim3dVbdGpuF32('inv_mass'),
    ).then((eng) => {
      if (eng) {
        webgpuRef.current = eng;
        setWebgpuActive(true);
      }
    });
  };

  // Strict lifecycle: initialize 3D engine on mount, dispose on unmount!
  useEffect(() => {
    publishStats(kernel.initSim3d(pose, bodyForm, meshSpacing, selfCollide));
    initWebGpuEngine();
    return () => {
      webgpuRef.current?.dispose();
      webgpuRef.current = null;
      kernel.disposeSim3d();
      useStudioStore.getState().setSim3dTelemetry(null, null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Pose, body form, mesh spacing and "re-drape" re-arrange the garment (skipped on mount).
  const arrangedRef = useRef(false);
  useEffect(() => {
    if (!arrangedRef.current) {
      arrangedRef.current = true;
      return;
    }
    publishStats(kernel.resetSim3d(pose, bodyForm, meshSpacing, selfCollide));
    if (windOn) kernel.setSim3dWind(1.8, 0.0, 0.6);
    initWebGpuEngine();
    lastVerRef.current = -1;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pose, bodyForm, meshSpacing, resetSeq]);

  // Auto-frame camera smoothly when template changes
  useEffect(() => {
    const fr = defaultFramingForTemplate(snapshot?.template_id);
    orbitRef.current.targetX = 0;
    orbitRef.current.targetY = fr.targetY;
    orbitRef.current.targetZ = 0;
    orbitRef.current.dist = fr.dist;
  }, [snapshot?.template_id]);

  // Apply Camera Preset clicks
  useEffect(() => {
    const fr = defaultFramingForTemplate(useStudioStore.getState().snapshot?.template_id);
    const orb = orbitRef.current;
    orb.targetX = 0;
    orb.targetY = fr.targetY;
    orb.targetZ = 0;
    orb.dist = fr.dist;
    if (cameraPreset === 'front') {
      orb.yaw = 0;
      orb.pitch = 0.05;
    } else if (cameraPreset === 'back') {
      orb.yaw = Math.PI;
      orb.pitch = 0.06;
    } else if (cameraPreset === 'left') {
      orb.yaw = Math.PI * 0.5;
      orb.pitch = 0.05;
    } else if (cameraPreset === 'right') {
      orb.yaw = -Math.PI * 0.5;
      orb.pitch = 0.05;
    } else if (cameraPreset === 'top') {
      orb.yaw = 0;
      orb.pitch = Math.PI * 0.5;
    } else if (cameraPreset === 'bottom') {
      orb.yaw = 0;
      orb.pitch = -Math.PI * 0.5;
    } else {
      orb.yaw = 0.36;
      orb.pitch = 0.08;
    }
  }, [cameraPreset, cameraSeq]);

  // Imperative WebGL2 Render + XPBD Step Loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: false });
    if (!gl) {
      setGlError('webgl2');
      return;
    }

    let prog: WebGLProgram;
    try {
      const vs = compileShader(gl, gl.VERTEX_SHADER, VS_SRC);
      const fs = compileShader(gl, gl.FRAGMENT_SHADER, FS_SRC);
      prog = gl.createProgram()!;
      gl.attachShader(prog, vs);
      gl.attachShader(prog, fs);
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
        throw new Error(gl.getProgramInfoLog(prog) || 'Program link failed');
      }
    } catch (e) {
      setGlError((e as Error).message);
      return;
    }

    const uMvp = gl.getUniformLocation(prog, 'u_mvp');
    const uMode = gl.getUniformLocation(prog, 'u_mode');
    const uBaseColor = gl.getUniformLocation(prog, 'u_baseColor');
    const uEyePos = gl.getUniformLocation(prog, 'u_eyePos');
    const uIsDark = gl.getUniformLocation(prog, 'u_isDark');
    const uTapeY = gl.getUniformLocation(prog, 'u_tapeY');
    const uNeckY = gl.getUniformLocation(prog, 'u_neckY');
    const uShowTape = gl.getUniformLocation(prog, 'u_showTape');
    const uFinish = gl.getUniformLocation(prog, 'u_finish');

    // Static studio floor quad at y = 0.0
    const floorVao = gl.createVertexArray()!;
    const fPosBuf = gl.createBuffer()!;
    const fNormBuf = gl.createBuffer()!;
    const fIdxBuf = gl.createBuffer()!;
    const floorSize = 8.0;
    const floorPos = new Float32Array([
      -floorSize, 0, -floorSize,
      floorSize, 0, -floorSize,
      floorSize, 0, floorSize,
      -floorSize, 0, floorSize,
    ]);
    const floorNorm = new Float32Array([0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0]);
    const floorIdx = new Uint32Array([0, 2, 1, 0, 3, 2]);

    gl.bindVertexArray(floorVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, fPosBuf);
    gl.bufferData(gl.ARRAY_BUFFER, floorPos, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, fNormBuf);
    gl.bufferData(gl.ARRAY_BUFFER, floorNorm, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);
    gl.disableVertexAttribArray(2);
    gl.vertexAttrib1f(2, 0.0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, fIdxBuf);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, floorIdx, gl.STATIC_DRAW);

    const avatarVao = gl.createVertexArray()!;
    const aPosBuf = gl.createBuffer()!;
    const aNormBuf = gl.createBuffer()!;
    const aMaskBuf = gl.createBuffer()!;
    const aIdxBuf = gl.createBuffer()!;

    const clothVao = gl.createVertexArray()!;
    const cPosBuf = gl.createBuffer()!;
    const cNormBuf = gl.createBuffer()!;
    const cMetricBuf = gl.createBuffer()!;
    const cColorBuf = gl.createBuffer()!;
    const cSeamBuf = gl.createBuffer()!;
    const cIdxBuf = gl.createBuffer()!;

    let avatarIdxCount = 0;
    let clothIdxCount = 0;
    let rafId = 0;
    let frameTick = 0;
    let lastTickMs = performance.now();

    // Avatar and cloth topology change only with the sim version; positions change every frame.
    const uploadAll = (full: boolean) => {
      const b = kernel.getSim3dBuffers();
      clothIdxCount = b.clothIdx.length;
      if (full) {
      avatarIdxCount = b.avatarIdx.length;
      gl.bindVertexArray(avatarVao);
      gl.bindBuffer(gl.ARRAY_BUFFER, aPosBuf);
      gl.bufferData(gl.ARRAY_BUFFER, b.avatarPos, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, aNormBuf);
      gl.bufferData(gl.ARRAY_BUFFER, b.avatarNorm, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(1);
      gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, aMaskBuf);
      gl.bufferData(gl.ARRAY_BUFFER, b.avatarMask, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(2);
      gl.vertexAttribPointer(2, 1, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, aIdxBuf);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, b.avatarIdx, gl.STATIC_DRAW);
      }

      const mode = useStudioStore.getState().sim3dViewMode;
      const metric =
        mode === 'strain'
          ? b.clothStrain
          : mode === 'tension'
            ? b.clothTension
            : b.clothClearance;

      gl.bindVertexArray(clothVao);
      gl.bindBuffer(gl.ARRAY_BUFFER, cPosBuf);
      gl.bufferData(gl.ARRAY_BUFFER, b.clothPos, gl.DYNAMIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, cNormBuf);
      gl.bufferData(gl.ARRAY_BUFFER, b.clothNorm, gl.DYNAMIC_DRAW);
      gl.enableVertexAttribArray(1);
      gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, cMetricBuf);
      gl.bufferData(gl.ARRAY_BUFFER, metric, gl.DYNAMIC_DRAW);
      gl.enableVertexAttribArray(2);
      gl.vertexAttribPointer(2, 1, gl.FLOAT, false, 0, 0);

      if (full) {
        gl.bindBuffer(gl.ARRAY_BUFFER, cColorBuf);
        gl.bufferData(gl.ARRAY_BUFFER, b.clothColor, gl.STATIC_DRAW);
        gl.enableVertexAttribArray(3);
        gl.vertexAttribPointer(3, 3, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, cSeamBuf);
        gl.bufferData(gl.ARRAY_BUFFER, b.clothSeamMask, gl.STATIC_DRAW);
        gl.enableVertexAttribArray(4);
        gl.vertexAttribPointer(4, 1, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, cIdxBuf);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, b.clothIdx, gl.STATIC_DRAW);
      }
      gl.bindVertexArray(null);
    };

    const tick = (now: number) => {
      rafId = requestAnimationFrame(tick);
      const dtSec = Math.min(0.05, Math.max(0.001, (now - lastTickMs) * 0.001));
      lastTickMs = now;
      frameTick += 1;

      const held = keysRef.current;
      if (held.size > 0) {
        const forward =
          (held.has('KeyW') || held.has('ArrowUp') ? 1 : 0) -
          (held.has('KeyS') || held.has('ArrowDown') ? 1 : 0);
        const right =
          (held.has('KeyD') || held.has('ArrowRight') ? 1 : 0) -
          (held.has('KeyA') || held.has('ArrowLeft') ? 1 : 0);
        const up =
          (held.has('KeyE') || held.has('PageUp') ? 1 : 0) -
          (held.has('KeyQ') || held.has('PageDown') ? 1 : 0);
        if (forward !== 0 || right !== 0 || up !== 0) {
          applyFreeMove(
            orbitRef.current,
            {
              forward,
              right,
              up,
              sprint: held.has('ShiftLeft') || held.has('ShiftRight'),
            },
            dtSec,
          );
        }
      }

      const ver = getSim3dVersion();
      if (ver !== lastVerRef.current) {
        lastVerRef.current = ver;
        const syncStats = kernel.stepSim3d(1);
        uploadAll(true);
        if (syncStats) {
          statsRef.current = syncStats;
          useStudioStore.getState().setSim3dTelemetry(syncStats);
        }
      } else if (playingRef.current) {
        const nextStats = kernel.stepSim3d(1);
        uploadAll(false);
        if (nextStats) statsRef.current = nextStats;
        if (nextStats && frameTick % 12 === 0) {
          const quality = frameTick % 96 === 0 ? kernel.getSim3dQuality() : undefined;
          useStudioStore.getState().setSim3dTelemetry(nextStats, quality);
        }
      }

      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const w = Math.max(1, Math.floor(rect.width * dpr));
      const h = Math.max(1, Math.floor(rect.height * dpr));
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }

      const st = useStudioStore.getState();
      const isDark = st.theme === 'dark';

      gl.viewport(0, 0, w, h);
      gl.clearColor(isDark ? 0.10 : 0.94, isDark ? 0.11 : 0.935, isDark ? 0.13 : 0.92, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      gl.enable(gl.DEPTH_TEST);

      const orb = orbitRef.current;
      const { eye } = cameraBasis(orb);
      const { near, far } = cameraClipPlanes(orb.dist);
      const proj = makePerspective(CAMERA_FOV_RAD, w / h, near, far);
      const view = makeViewMatrix(orb);
      const mvp = mulMat4(proj, view);

      gl.useProgram(prog);
      gl.uniformMatrix4fv(uMvp, false, mvp);
      gl.uniform3f(uEyePos, eye[0], eye[1], eye[2]);
      gl.uniform1f(uIsDark, isDark ? 1.0 : 0.0);

      // 1. Studio Floor Grid + Contact Shadow
      if (st.show3dFloor) {
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
        gl.uniform1i(uMode, 6);
        gl.bindVertexArray(floorVao);
        gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_INT, 0);
        gl.disable(gl.BLEND);
      }

      // 2. Sculpted Parametric Studio Mannequin
      if (st.show3dAvatar && avatarIdxCount > 0) {
        const ls = statsRef.current;
        gl.disable(gl.BLEND);
        gl.uniform1i(uMode, 0);
        gl.uniform3f(uBaseColor, isDark ? 0.44 : 0.66, isDark ? 0.44 : 0.67, isDark ? 0.47 : 0.70);
        gl.uniform4f(
          uTapeY,
          ls?.bust_y ?? 1.33,
          ls?.waist_y ?? 1.12,
          ls?.hip_y ?? 0.92,
          ls?.crotch_y ?? 0.85,
        );
        gl.uniform1f(uNeckY, ls?.neck_y ?? 1.60);
        gl.uniform1f(uShowTape, st.show3dTapeLines ? 1.0 : 0.0);
        gl.bindVertexArray(avatarVao);
        gl.drawElements(gl.TRIANGLES, avatarIdxCount, gl.UNSIGNED_INT, 0);
      }

      // 3. Simulated 3D Garment
      if (clothIdxCount > 0) {
        const mode = st.sim3dViewMode;
        const modeInt =
          mode === 'material'
            ? 1
            : mode === 'strain'
              ? 2
              : mode === 'tension'
                ? 3
                : mode === 'clearance'
                  ? 4
                  : 5;

        const finish = st.snapshot?.fabric.finish ?? 'plain';
        if (modeInt === 5 || (modeInt === 1 && finish === 'sheer')) {
          gl.enable(gl.BLEND);
          gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
          gl.depthMask(false);
        }
        const [cr, cg, cb] = hexToRgb01(st.snapshot?.fabric.color_hex || '#2d4c73');
        gl.uniform1i(uMode, modeInt);
        gl.uniform1i(uFinish, FINISH_CODE[finish] ?? 0);
        gl.uniform3f(uBaseColor, cr, cg, cb);
        gl.bindVertexArray(clothVao);
        gl.drawElements(gl.TRIANGLES, clothIdxCount, gl.UNSIGNED_INT, 0);
        gl.depthMask(true);
        gl.disable(gl.BLEND);
      }
      gl.bindVertexArray(null);
    };

    rafId = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(rafId);
      gl.deleteBuffer(fPosBuf);
      gl.deleteBuffer(fNormBuf);
      gl.deleteBuffer(fIdxBuf);
      gl.deleteVertexArray(floorVao);
      gl.deleteBuffer(aPosBuf);
      gl.deleteBuffer(aNormBuf);
      gl.deleteBuffer(aMaskBuf);
      gl.deleteBuffer(aIdxBuf);
      gl.deleteBuffer(cPosBuf);
      gl.deleteBuffer(cNormBuf);
      gl.deleteBuffer(cMetricBuf);
      gl.deleteBuffer(cColorBuf);
      gl.deleteBuffer(cSeamBuf);
      gl.deleteBuffer(cIdxBuf);
      gl.deleteVertexArray(avatarVao);
      gl.deleteVertexArray(clothVao);
      gl.deleteProgram(prog);
    };
  }, []);

  const zoomStepSeq = useStudioStore((s) => s.zoomStepSeq);
  const fitViewSeq = useStudioStore((s) => s.fitViewSeq);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);

  useEffect(() => {
    lastVerRef.current = -1;
  }, [viewMode]);

  useEffect(() => {
    if (zoomStepSeq === 0 || workspaceMode !== '3d') return;
    const factor = useStudioStore.getState().zoomStepFactor;
    applyZoomFactor(orbitRef.current, factor);
  }, [zoomStepSeq, workspaceMode]);

  useEffect(() => {
    if (fitViewSeq === 0 || workspaceMode !== '3d') return;
    const fr = defaultFramingForTemplate(useStudioStore.getState().snapshot?.template_id);
    const orb = orbitRef.current;
    orb.targetX = 0;
    orb.targetY = fr.targetY;
    orb.targetZ = 0;
    orb.dist = fr.dist;
  }, [fitViewSeq, workspaceMode]);

  useEffect(() => {
    const isEditable = (el: EventTarget | null) =>
      el instanceof HTMLInputElement ||
      el instanceof HTMLTextAreaElement ||
      el instanceof HTMLSelectElement ||
      (el instanceof HTMLElement && el.isContentEditable);

    const onKeyDown = (e: KeyboardEvent) => {
      if (isEditable(e.target) || e.metaKey || e.ctrlKey || e.altKey) return;
      const mode = useStudioStore.getState().workspaceMode;
      if (mode !== '3d' && document.activeElement !== canvasRef.current) return;
      keysRef.current.add(e.code);
    };
    const onKeyUp = (e: KeyboardEvent) => {
      keysRef.current.delete(e.code);
    };
    const onBlur = () => {
      keysRef.current.clear();
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const factor = wheelZoomFactor(e);
      applyZoomFactor(orbitRef.current, factor);
    };
    canvas.addEventListener('wheel', onWheel, { passive: false });
    return () => canvas.removeEventListener('wheel', onWheel);
  }, [glError]);

  const showLegend = viewMode === 'strain' || viewMode === 'tension' || viewMode === 'clearance';

  const navBtn = (active: boolean): React.CSSProperties => ({
    background: active ? 'var(--accent-soft)' : undefined,
    color: active ? 'var(--accent)' : 'var(--text-secondary)',
  });

  return (
    <div
      style={{ ...style, background: 'var(--bg-canvas)' }}
      className="relative h-full min-w-[260px] overflow-hidden select-none"
    >
      {glError ? (
        <div className="p-4 text-xs text-rose-500 font-mono-num">
          {glError === 'webgl2' ? t('v3d.webglMissing') : glError}
        </div>
      ) : (
        <canvas
          ref={canvasRef}
          tabIndex={0}
          data-viewport3d="true"
          onContextMenu={(e) => e.preventDefault()}
          onDoubleClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const ray = unprojectCameraRay(e.clientX, e.clientY, rect, orbitRef.current);
            const hit = kernel.pickSim3dVertex(
              ray.origin[0],
              ray.origin[1],
              ray.origin[2],
              ray.dir[0],
              ray.dir[1],
              ray.dir[2],
            );
            const orb = orbitRef.current;
            if (hit) {
              orb.targetX = hit.pos[0];
              orb.targetY = hit.pos[1];
              orb.targetZ = hit.pos[2];
              orb.dist = Math.min(orb.dist, 0.85);
            } else {
              const fr = defaultFramingForTemplate(useStudioStore.getState().snapshot?.template_id);
              orb.targetX = 0;
              orb.targetY = fr.targetY;
              orb.targetZ = 0;
              orb.dist = fr.dist;
            }
          }}
          onPointerDown={(e) => {
            e.currentTarget.focus({ preventScroll: true });
            e.currentTarget.setPointerCapture(e.pointerId);
            navRef.current.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
            if (navRef.current.pointers.size >= 2) {
              if (grabRef.current) {
                grabRef.current = null;
                kernel.grabSim3dEnd();
              }
              navRef.current.mode = 'none';
              return;
            }
            if ((grabMode || e.altKey) && e.button === 0) {
              const rect = e.currentTarget.getBoundingClientRect();
              const ray = unprojectCameraRay(e.clientX, e.clientY, rect, orbitRef.current);
              const hit = kernel.pickSim3dVertex(
                ray.origin[0],
                ray.origin[1],
                ray.origin[2],
                ray.dir[0],
                ray.dir[1],
                ray.dir[2],
              );
              if (hit) {
                if (e.shiftKey) {
                  kernel.toggleSim3dPin(hit.vert);
                  const st = kernel.stepSim3d(1);
                  if (st) publishStats(st);
                  return;
                }
                kernel.grabSim3dStart(hit.vert, hit.pos[0], hit.pos[1], hit.pos[2], 0.14);
                grabRef.current = {
                  planePoint: hit.pos,
                  planeNormal: ray.fwd,
                };
                if (!playingRef.current) setPlaying(true);
                return;
              }
            }
            const isPan = e.button === 2 || e.button === 1 || e.shiftKey;
            navRef.current.mode = isPan ? 'pan' : 'orbit';
            navRef.current.yawSign = Math.cos(orbitRef.current.pitch) >= 0 ? 1 : -1;
            navRef.current.lx = e.clientX;
            navRef.current.ly = e.clientY;
          }}
          onPointerMove={(e) => {
            const ptrs = navRef.current.pointers;
            const prevPtr = ptrs.get(e.pointerId);
            if (ptrs.size === 2 && prevPtr) {
              const pts = Array.from(ptrs.entries());
              const other = pts[0][0] === e.pointerId ? pts[1][1] : pts[0][1];
              const prevMidX = 0.5 * (prevPtr.x + other.x);
              const prevMidY = 0.5 * (prevPtr.y + other.y);
              const prevDist = Math.hypot(prevPtr.x - other.x, prevPtr.y - other.y);
              ptrs.set(e.pointerId, { x: e.clientX, y: e.clientY });
              const nextMidX = 0.5 * (e.clientX + other.x);
              const nextMidY = 0.5 * (e.clientY + other.y);
              const nextDist = Math.hypot(e.clientX - other.x, e.clientY - other.y);
              const rect = e.currentTarget.getBoundingClientRect();
              applyPanDelta(orbitRef.current, nextMidX - prevMidX, nextMidY - prevMidY, rect.height);
              if (prevDist > 8 && nextDist > 8) {
                applyZoomFactor(orbitRef.current, nextDist / prevDist);
              }
              return;
            }
            if (prevPtr) {
              ptrs.set(e.pointerId, { x: e.clientX, y: e.clientY });
            }
            if (grabRef.current) {
              const rect = e.currentTarget.getBoundingClientRect();
              const ray = unprojectCameraRay(e.clientX, e.clientY, rect, orbitRef.current);
              const { planePoint: pp, planeNormal: pn } = grabRef.current;
              const denom = ray.dir[0] * pn[0] + ray.dir[1] * pn[1] + ray.dir[2] * pn[2];
              if (Math.abs(denom) > 1e-4) {
                const tHit =
                  ((pp[0] - ray.origin[0]) * pn[0] +
                    (pp[1] - ray.origin[1]) * pn[1] +
                    (pp[2] - ray.origin[2]) * pn[2]) /
                  denom;
                if (tHit > 0.05 && tHit < 10) {
                  kernel.grabSim3dMove(
                    ray.origin[0] + ray.dir[0] * tHit,
                    ray.origin[1] + ray.dir[1] * tHit,
                    ray.origin[2] + ray.dir[2] * tHit,
                  );
                }
              }
              return;
            }
            if (navRef.current.mode === 'none') return;
            const dx = e.clientX - navRef.current.lx;
            const dy = e.clientY - navRef.current.ly;
            navRef.current.lx = e.clientX;
            navRef.current.ly = e.clientY;
            if (navRef.current.mode === 'pan') {
              const rect = e.currentTarget.getBoundingClientRect();
              applyPanDelta(orbitRef.current, dx, dy, rect.height);
            } else {
              applyOrbitDelta(orbitRef.current, dx, dy, navRef.current.yawSign);
            }
          }}
          onPointerUp={(e) => {
            navRef.current.pointers.delete(e.pointerId);
            if (grabRef.current) {
              grabRef.current = null;
              kernel.grabSim3dEnd();
            }
            navRef.current.mode = 'none';
          }}
          onPointerCancel={(e) => {
            navRef.current.pointers.delete(e.pointerId);
            if (grabRef.current) {
              grabRef.current = null;
              kernel.grabSim3dEnd();
            }
            navRef.current.mode = 'none';
          }}
          className={`w-full h-full block touch-none outline-none ${
            grabMode ? 'cursor-crosshair' : 'cursor-grab active:cursor-grabbing'
          }`}
        />
      )}

      {/* Heatmap legend (strain / tension / ease views) */}
      {showLegend && (
        <div className="floating-panel absolute bottom-2 left-2 z-10 px-2.5 py-1.5 text-[10px] font-mono-num">
          <div className="mb-1 font-sans font-semibold text-[10px]">
            {viewMode === 'strain'
              ? t('v3d.legendStrain')
              : viewMode === 'tension'
                ? t('v3d.legendTension')
                : t('v3d.legendEase')}
          </div>
          <div
            className="w-28 h-2 rounded-[2px]"
            style={{
              background:
                viewMode === 'clearance'
                  ? 'linear-gradient(90deg, #e0292e, #f2ad1f, #14bd80, #1a7ae6)'
                  : 'linear-gradient(90deg, #1a7ae6, #14bd80, #f2ad1f, #e0292e)',
            }}
          />
          <div className="flex justify-between mt-0.5 text-[9px]" style={{ color: 'var(--text-muted)' }}>
            <span>{viewMode === 'clearance' ? t('v3d.tight') : '0'}</span>
            <span>{viewMode === 'clearance' ? t('v3d.ideal') : viewMode === 'strain' ? '6%' : '80'}</span>
            <span>{viewMode === 'clearance' ? t('v3d.loose') : viewMode === 'strain' ? '12%+' : '160+'}</span>
          </div>
        </div>
      )}

      {/* Solver & Mesh Resolution HUD (top-right) */}
      <div className="floating-panel absolute top-2 right-2 z-10 h-[26px] px-2 flex items-center gap-2 text-[10px] font-mono-num">
        <span style={{ color: webgpuActive ? '#0e9f6e' : 'var(--text-secondary)' }}>
          {webgpuActive ? 'VBD · WebGPU' : 'XPBD+VBD · WASM'}
        </span>
        {statsRef.current?.adaptive_mesh && statsRef.current.max_edge_mm > 0 && (
          <span style={{ color: 'var(--text-muted)' }}>
            {statsRef.current.min_edge_mm.toFixed(0)}–{statsRef.current.max_edge_mm.toFixed(0)} mm
          </span>
        )}
        <select
          value={meshSpacing}
          onChange={(e) => setMeshSpacing(Number(e.target.value))}
          title={t('v3d.meshSpacingTitle')}
          aria-label={t('v3d.meshSpacingTitle')}
          className="bg-transparent text-[10px] cursor-pointer outline-none"
          style={{ color: 'var(--text-primary)' }}
        >
          <option value={0}>{t('v3d.meshAuto')}</option>
          <option value={20}>{t('v3d.mesh20')}</option>
          <option value={14}>{t('v3d.mesh14')}</option>
          <option value={10}>{t('v3d.mesh10')}</option>
          <option value={7}>{t('v3d.mesh7')}</option>
        </select>
      </div>

      {/* Navigation bar (bottom centre): camera views, scene toggles, simulation */}
      <div
        className="floating-panel absolute bottom-2 left-1/2 -translate-x-1/2 z-10 h-[30px] px-1 flex items-center gap-0.5 text-[11px] whitespace-nowrap"
        role="toolbar"
        aria-label={t('nav.title3d')}
      >
        {(
          [
            ['front', 'v3d.front'],
            ['three_quarter', 'v3d.threeQuarter'],
            ['left', 'v3d.side'],
            ['back', 'v3d.back'],
            ['top', 'v3d.top'],
            ['bottom', 'v3d.bottom'],
          ] as Array<[Camera3DPreset, MessageKey]>
        ).map(([p, lbl]) => (
          <button
            key={p}
            onClick={() => request3dCamera(p)}
            className="h-[24px] px-1.5 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
            style={navBtn(cameraPreset === p)}
            title={t('nav.camera', { view: t(lbl) })}
          >
            {t(lbl)}
          </button>
        ))}
        <button
          onClick={() => {
            const fr = defaultFramingForTemplate(useStudioStore.getState().snapshot?.template_id);
            const orb = orbitRef.current;
            orb.targetX = 0;
            orb.targetY = fr.targetY;
            orb.targetZ = 0;
            orb.dist = fr.dist;
            orb.yaw = 0.36;
            orb.pitch = 0.08;
          }}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{ color: 'var(--text-secondary)' }}
          title={t('v3d.reframe')}
          aria-label={t('v3d.reframe')}
        >
          <Focus className="w-3.5 h-3.5" />
        </button>
        <div className="w-px h-4 mx-1" style={{ background: 'var(--border-subtle)' }} />
        <button
          onClick={toggle3dAvatar}
          aria-pressed={showAvatar}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(showAvatar)}
          title={t('v3d.avatarTitle')}
        >
          <PersonStanding className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={toggle3dTapeLines}
          aria-pressed={showTape}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(showTape)}
          title={t('v3d.tapeTitle')}
        >
          <Ruler className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={toggle3dFloor}
          aria-pressed={showFloor}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(showFloor)}
          title={t('v3d.floorTitle')}
        >
          <Grid3x3 className="w-3.5 h-3.5" />
        </button>
        <div className="w-px h-4 mx-1" style={{ background: 'var(--border-subtle)' }} />
        <button
          onClick={() => setPlaying(!playing)}
          aria-pressed={playing}
          className="h-[24px] px-1.5 flex items-center gap-1 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{ color: playing ? '#0e9f6e' : 'var(--text-secondary)' }}
          title={playing ? t('v3d.pause') : t('v3d.play')}
        >
          {playing ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
          {playing ? t('nav.pause') : t('nav.simulate')}
        </button>
        <button
          onClick={requestSim3dReset}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{ color: 'var(--text-secondary)' }}
          title={t('v3d.reset')}
          aria-label={t('v3d.reset')}
        >
          <RotateCcw className="w-3.5 h-3.5" />
        </button>
        <div className="w-px h-4 mx-1" style={{ background: 'var(--border-subtle)' }} />
        <button
          onClick={() => setGrabMode(!grabMode)}
          aria-pressed={grabMode}
          className="h-[24px] px-1.5 flex items-center gap-1 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(grabMode)}
          title={t('v3d.grabModeTitle')}
        >
          <Hand className="w-3.5 h-3.5" />
          {t('v3d.grabMode')}
        </button>
        {pinCount > 0 && (
          <button
            onClick={() => {
              kernel.clearSim3dPins();
              setPinCount(0);
            }}
            className="h-[24px] px-1.5 flex items-center gap-1 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)] text-rose-500"
            title={t('v3d.clearPins', { count: pinCount })}
          >
            <X className="w-3 h-3" />
            {pinCount}
          </button>
        )}
        <button
          onClick={() => {
            const next = !selfCollide;
            setSelfCollide(next);
            kernel.setSim3dSelfCollision(next);
          }}
          aria-pressed={selfCollide}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(selfCollide)}
          title={t('v3d.selfCollideTitle')}
        >
          <Layers className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => {
            const next = !windOn;
            setWindOn(next);
            kernel.setSim3dWind(next ? 1.8 : 0.0, 0.0, next ? 0.6 : 0.0);
          }}
          aria-pressed={windOn}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={navBtn(windOn)}
          title={t('v3d.windTitle')}
        >
          <Wind className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Subtle navigation hint (bottom-right, desktop) */}
      <div
        className="pointer-events-none hidden xl:block absolute bottom-2 right-2 z-10 px-2 py-0.5 rounded text-[10px]"
        style={{ color: 'var(--text-muted)', background: 'color-mix(in srgb, var(--bg-panel) 72%, transparent)' }}
      >
        {t('v3d.navHint')}
      </div>
    </div>
  );
};

export default Viewport3D;
