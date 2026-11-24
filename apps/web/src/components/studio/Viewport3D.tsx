import React, { useEffect, useRef, useState } from 'react';
import { Eye, EyeOff, Grid, Pause, Play, RotateCcw, Ruler, X } from 'lucide-react';
import { getSim3dVersion, kernel } from '../../lib/kernelClient';
import { useCatalogT, useT, type MessageKey } from '../../lib/i18n';
import type {
  AvatarBodyForm,
  AvatarPose,
  Camera3DPreset,
  DrapeQuality,
  FabricFinish,
  FitViewMode,
  Sim3dStats,
} from '../../lib/types';

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

const VS_SRC = `#version 300 es
precision highp float;
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;
layout(location = 2) in float a_metric;
layout(location = 3) in vec3 a_color;

uniform mat4 u_mvp;
out vec3 v_norm;
out vec3 v_pos;
out float v_metric;
out vec3 v_color;

void main() {
  v_norm = a_norm;
  v_pos = a_pos;
  v_metric = a_metric;
  v_color = a_color;
  gl_Position = u_mvp * vec4(a_pos, 1.0);
}
`;

const FS_SRC = `#version 300 es
precision highp float;
in vec3 v_norm;
in vec3 v_pos;
in float v_metric;
in vec3 v_color;

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
    float fade = 1.0 - smoothstep(0.65, 2.2, r);
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
function makeLookAt(eye: [number, number, number], target: [number, number, number]): Float32Array {
  const zx = eye[0] - target[0];
  const zy = eye[1] - target[1];
  const zz = eye[2] - target[2];
  const zl = Math.hypot(zx, zy, zz) || 1;
  const fz = [zx / zl, zy / zl, zz / zl];

  // up = (0, 1, 0) -> up x fz = (1*fz[2] - 0*fz[1], 0*fz[0] - 0*fz[2], 0*fz[1] - 1*fz[0]) = (fz[2], 0, -fz[0])
  const rx = fz[2];
  const ry = 0;
  const rz = -fz[0];
  const rl = Math.hypot(rx, ry, rz) || 1;
  const fx = [rx / rl, ry / rl, rz / rl];

  // camUp = fz x fx
  const fy = [
    fz[1] * fx[2] - fz[2] * fx[1],
    fz[2] * fx[0] - fz[0] * fx[2],
    fz[0] * fx[1] - fz[1] * fx[0],
  ];

  return new Float32Array([
    fx[0], fy[0], fz[0], 0,
    fx[1], fy[1], fz[1], 0,
    fx[2], fy[2], fz[2], 0,
    -(fx[0] * eye[0] + fx[1] * eye[1] + fx[2] * eye[2]),
    -(fy[0] * eye[0] + fy[1] * eye[1] + fy[2] * eye[2]),
    -(fz[0] * eye[0] + fz[1] * eye[1] + fz[2] * eye[2]),
    1,
  ]);
}

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
  if (templateId === 'trouser_block') {
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
  const orbitRef = useRef({
    yaw: 0.36,
    pitch: 0.08,
    dist: initialFrame.dist,
    targetY: initialFrame.targetY,
    dragging: false,
    lx: 0,
    ly: 0,
  });
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

  const setSim3dPose = useStudioStore((s) => s.setSim3dPose);
  const setSim3dBodyForm = useStudioStore((s) => s.setSim3dBodyForm);
  const setSim3dViewMode = useStudioStore((s) => s.setSim3dViewMode);
  const toggle3dAvatar = useStudioStore((s) => s.toggle3dAvatar);
  const toggle3dFloor = useStudioStore((s) => s.toggle3dFloor);
  const toggle3dTapeLines = useStudioStore((s) => s.toggle3dTapeLines);
  const request3dCamera = useStudioStore((s) => s.request3dCamera);
  const setWorkspaceMode = useStudioStore((s) => s.setWorkspaceMode);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);

  const [playing, setPlaying] = useState(true);
  const [stats, setStats] = useState<Sim3dStats | null>(null);
  const statsRef = useRef<Sim3dStats | null>(null);
  const [glError, setGlError] = useState<string | null>(null);
  const [quality, setQuality] = useState<DrapeQuality | null>(null);
  const t = useT();
  const ct = useCatalogT();

  useEffect(() => {
    statsRef.current = stats;
  }, [stats]);

  useEffect(() => {
    playingRef.current = playing;
  }, [playing]);

  // Strict lifecycle: initialize 3D engine on mount, dispose on unmount!
  useEffect(() => {
    const initialStats = kernel.initSim3d(pose, bodyForm);
    statsRef.current = initialStats;
    setStats(initialStats);
    return () => {
      kernel.disposeSim3d();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Auto-frame camera smoothly when template changes
  useEffect(() => {
    const fr = defaultFramingForTemplate(snapshot?.template_id);
    orbitRef.current.targetY = fr.targetY;
    orbitRef.current.dist = fr.dist;
  }, [snapshot?.template_id]);

  // Apply Camera Preset clicks
  useEffect(() => {
    const fr = defaultFramingForTemplate(useStudioStore.getState().snapshot?.template_id);
    const orb = orbitRef.current;
    orb.targetY = fr.targetY;
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
    } else {
      orb.yaw = 0.36;
      orb.pitch = 0.08;
    }
  }, [cameraPreset, cameraSeq]);

  const handlePoseChange = (nextPose: AvatarPose) => {
    setSim3dPose(nextPose);
    const nextStats = kernel.resetSim3d(nextPose, bodyForm);
    statsRef.current = nextStats;
    setStats(nextStats);
    lastVerRef.current = -1;
  };

  const handleBodyFormChange = (nextForm: AvatarBodyForm) => {
    setSim3dBodyForm(nextForm);
    const nextStats = kernel.resetSim3d(pose, nextForm);
    statsRef.current = nextStats;
    setStats(nextStats);
    lastVerRef.current = -1;
  };

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
    const floorSize = 2.4;
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
    const cIdxBuf = gl.createBuffer()!;

    let avatarIdxCount = 0;
    let clothIdxCount = 0;
    let rafId = 0;
    let frameTick = 0;

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
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, cIdxBuf);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, b.clothIdx, gl.STATIC_DRAW);
      }
      gl.bindVertexArray(null);
    };

    const tick = () => {
      rafId = requestAnimationFrame(tick);
      frameTick += 1;

      const ver = getSim3dVersion();
      if (ver !== lastVerRef.current) {
        lastVerRef.current = ver;
        const syncStats = kernel.stepSim3d(1);
        uploadAll(true);
        if (syncStats) {
          statsRef.current = syncStats;
          setStats(syncStats);
        }
      } else if (playingRef.current) {
        const nextStats = kernel.stepSim3d(1);
        uploadAll(false);
        if (nextStats && frameTick % 12 === 0) {
          statsRef.current = nextStats;
          setStats(nextStats);
        }
        if (frameTick % 90 === 0) {
          setQuality(kernel.getSim3dQuality());
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
      const cp = Math.cos(orb.pitch);
      const eye: [number, number, number] = [
        orb.dist * cp * Math.sin(orb.yaw),
        orb.targetY + orb.dist * Math.sin(orb.pitch),
        orb.dist * cp * Math.cos(orb.yaw),
      ];
      const proj = makePerspective(0.62, w / h, 0.05, 18.0);
      const view = makeLookAt(eye, [0, orb.targetY, 0]);
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
      gl.deleteBuffer(cIdxBuf);
      gl.deleteVertexArray(avatarVao);
      gl.deleteVertexArray(clothVao);
      gl.deleteProgram(prog);
    };
  }, []);

  useEffect(() => {
    lastVerRef.current = -1;
  }, [viewMode]);

  const showLegend = viewMode === 'strain' || viewMode === 'tension' || viewMode === 'clearance';

  return (
    <div
      style={{ ...style, background: 'var(--bg-canvas)' }}
      className="relative h-full min-w-[260px] flex flex-col overflow-hidden select-none"
    >
      {/* Sleek Compact 3D Window Bar */}
      <div
        className="h-9 px-2.5 border-b flex items-center justify-between gap-2 text-[11px] shrink-0"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
      >
        <div className="flex items-center gap-1.5 min-w-0">
          <span className="font-semibold shrink-0">{t('v3d.title')}</span>
          <div className="flex items-center gap-0.5 rounded-md border p-0.5" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}>
            {(
              [
                ['material', 'v3d.modeDrape'],
                ['strain', 'v3d.modeStrain'],
                ['tension', 'v3d.modeTension'],
                ['clearance', 'v3d.modeEase'],
                ['xray', 'v3d.modeXray'],
              ] as Array<[FitViewMode, MessageKey]>
            ).map(([id, label]) => (
              <button
                key={id}
                onClick={() => setSim3dViewMode(id)}
                className="px-1.5 py-0.5 rounded text-[10px] font-medium cursor-pointer transition"
                style={{
                  background: viewMode === id ? 'var(--bg-surface)' : 'transparent',
                  color: viewMode === id ? 'var(--accent)' : 'var(--text-secondary)',
                }}
              >
                {t(label)}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          <select
            value={bodyForm}
            onChange={(e) => handleBodyFormChange(e.target.value as AvatarBodyForm)}
            className="rounded border px-1.5 py-0.5 text-[10px]"
            style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
            title={t('v3d.bodyForm')}
          >
            <option value="female">{t('v3d.female')}</option>
            <option value="neutral">{t('v3d.neutral')}</option>
            <option value="male">{t('v3d.male')}</option>
          </select>

          <select
            value={pose}
            onChange={(e) => handlePoseChange(e.target.value as AvatarPose)}
            className="rounded border px-1.5 py-0.5 text-[10px]"
            style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
            title={t('v3d.pose')}
          >
            <option value="a_pose">{t('v3d.poseA')}</option>
            <option value="arms_raised">{t('v3d.poseArmsUp')}</option>
            <option value="t_pose">{t('v3d.poseT')}</option>
          </select>

          {snapshot && (
            <select
              value={snapshot.fabric.id}
              onChange={(e) => setSnapshot(kernel.setFabricPreset(e.target.value))}
              className="hidden xl:block rounded border px-1.5 py-0.5 text-[10px] max-w-[116px]"
              style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              title={t('v3d.fabricPreset')}
            >
              {snapshot.fabric_catalog.map((f) => (
                <option key={f.id} value={f.id}>
                  {ct('fabricPreset', f.id, f.name).split(' (')[0]}
                </option>
              ))}
            </select>
          )}

          <button
            onClick={() => setPlaying((p) => !p)}
            className="p-1 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{
              borderColor: playing ? '#0e9f6e' : 'var(--border-subtle)',
              color: playing ? '#0e9f6e' : 'var(--text-secondary)',
            }}
            title={playing ? t('v3d.pause') : t('v3d.play')}
          >
            {playing ? <Pause className="w-3 h-3" /> : <Play className="w-3 h-3" />}
          </button>
          <button
            onClick={() => {
              const nextStats = kernel.resetSim3d(pose, bodyForm);
              statsRef.current = nextStats;
              setStats(nextStats);
              lastVerRef.current = -1;
            }}
            className="p-1 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
            title={t('v3d.reset')}
          >
            <RotateCcw className="w-3 h-3" />
          </button>
          <button
            onClick={() => setWorkspaceMode('2d')}
            className="p-1 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
            title={t('v3d.close')}
          >
            <X className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Imperative WebGL2 Canvas */}
      <div className="relative flex-1 min-h-0">
        {glError ? (
          <div className="p-4 text-xs text-rose-500 font-mono-num">
            {glError === 'webgl2' ? t('v3d.webglMissing') : glError}
          </div>
        ) : (
          <canvas
            ref={canvasRef}
            onPointerDown={(e) => {
              e.currentTarget.setPointerCapture(e.pointerId);
              orbitRef.current.dragging = true;
              orbitRef.current.lx = e.clientX;
              orbitRef.current.ly = e.clientY;
            }}
            onPointerMove={(e) => {
              if (!orbitRef.current.dragging) return;
              const dx = e.clientX - orbitRef.current.lx;
              const dy = e.clientY - orbitRef.current.ly;
              orbitRef.current.lx = e.clientX;
              orbitRef.current.ly = e.clientY;
              if (e.shiftKey || e.buttons === 4) {
                orbitRef.current.targetY = Math.max(
                  0.25,
                  Math.min(1.55, orbitRef.current.targetY + dy * 0.0028),
                );
              } else {
                orbitRef.current.yaw -= dx * 0.0085;
                orbitRef.current.pitch = Math.max(
                  -0.75,
                  Math.min(1.05, orbitRef.current.pitch + dy * 0.0075),
                );
              }
            }}
            onPointerUp={() => {
              orbitRef.current.dragging = false;
            }}
            onWheel={(e) => {
              e.preventDefault();
              orbitRef.current.dist = Math.max(
                0.72,
                Math.min(3.8, orbitRef.current.dist * (e.deltaY > 0 ? 1.08 : 0.92)),
              );
            }}
            className="w-full h-full block cursor-grab active:cursor-grabbing touch-none"
          />
        )}

        {/* CLO3D-Style Floating Camera & Display Overlay (Top-Left inside 3D stage) */}
        <div className="absolute top-2.5 left-2.5 flex flex-wrap items-center gap-1">
          <div
            className="flex items-center gap-0.5 rounded-lg border px-1 py-0.5 text-[10px] shadow-xs backdrop-blur-md"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', opacity: 0.92 }}
          >
            {(
              [
                ['front', 'v3d.front'],
                ['three_quarter', 'v3d.threeQuarter'],
                ['left', 'v3d.side'],
                ['back', 'v3d.back'],
              ] as Array<[Camera3DPreset, MessageKey]>
            ).map(([p, lbl]) => (
              <button
                key={p}
                onClick={() => request3dCamera(p)}
                className="px-1.5 py-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer font-medium"
                style={{ color: cameraPreset === p ? 'var(--accent)' : 'var(--text-secondary)' }}
              >
                {t(lbl)}
              </button>
            ))}
          </div>
          <button
            onClick={toggle3dAvatar}
            className="px-2 py-1 rounded-lg border text-[10px] inline-flex items-center gap-1 shadow-xs backdrop-blur-md cursor-pointer"
            style={{
              background: 'var(--bg-surface)',
              borderColor: showAvatar ? 'var(--border-subtle)' : 'var(--accent)',
              color: showAvatar ? 'var(--text-secondary)' : 'var(--accent)',
              opacity: 0.92,
            }}
            title={t('v3d.avatarTitle')}
          >
            {showAvatar ? <Eye className="w-3 h-3" /> : <EyeOff className="w-3 h-3" />}
            {t('v3d.avatar')}
          </button>
          <button
            onClick={toggle3dTapeLines}
            className="px-2 py-1 rounded-lg border text-[10px] inline-flex items-center gap-1 shadow-xs backdrop-blur-md cursor-pointer"
            style={{
              background: 'var(--bg-surface)',
              borderColor: showTape ? 'var(--accent)' : 'var(--border-subtle)',
              color: showTape ? 'var(--accent)' : 'var(--text-secondary)',
              opacity: 0.92,
            }}
            title={t('v3d.tapeTitle')}
          >
            <Ruler className="w-3 h-3" />
            {t('v3d.tape')}
          </button>
          <button
            onClick={toggle3dFloor}
            className="p-1 rounded-lg border shadow-xs backdrop-blur-md cursor-pointer"
            style={{
              background: 'var(--bg-surface)',
              borderColor: showFloor ? 'var(--accent)' : 'var(--border-subtle)',
              color: showFloor ? 'var(--accent)' : 'var(--text-secondary)',
              opacity: 0.92,
            }}
            title={t('v3d.floorTitle')}
          >
            <Grid className="w-3 h-3" />
          </button>
        </div>

        {/* Heatmap Scale Bar (when in Strain / Tension / Ease mode) */}
        {showLegend && (
          <div
            className="absolute top-2.5 right-2.5 rounded-lg border px-2.5 py-1.5 text-[10px] font-mono-num backdrop-blur-md shadow-xs"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', opacity: 0.94 }}
          >
            <div className="mb-1 font-sans font-semibold text-[10px]">
              {viewMode === 'strain'
                ? t('v3d.legendStrain')
                : viewMode === 'tension'
                  ? t('v3d.legendTension')
                  : t('v3d.legendEase')}
            </div>
            <div
              className="w-28 h-2 rounded-full"
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

        {/* Compact Bottom Telemetry Bar */}
        {stats && (
          <div
            className="absolute bottom-2.5 left-2.5 right-2.5 rounded-lg border px-2.5 py-1 flex flex-wrap items-center justify-between gap-2 text-[10px] font-mono-num backdrop-blur-md"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', opacity: 0.92 }}
          >
            <span style={{ color: 'var(--text-secondary)' }}>
              {t('v3d.telemetry', {
                tris: stats.triangle_count,
                stitches: stats.sew_pair_count,
                darts: stats.darts_closed ?? 0,
                height: (stats.measured_height_mm / 10).toFixed(0),
                bust: stats.measured_bust_mm.toFixed(0),
                waist: stats.measured_waist_mm.toFixed(0),
                hip: stats.measured_hip_mm.toFixed(0),
              })}
            </span>
            <span>
              {t('v3d.peakStrain')}{' '}
              <strong className={stats.max_strain_pct > 15 ? 'text-amber-600' : 'text-emerald-600'}>
                {stats.max_strain_pct.toFixed(1)}%
              </strong>{' '}
              · {t('v3d.easeShort', { ease: stats.avg_clearance_mm.toFixed(0) })}
              {quality && (
                <>
                  {' · '}
                  <span className={quality.poke_through_count > 0 ? 'text-amber-600' : 'text-emerald-600'}>
                    {quality.poke_through_count > 0
                      ? t('v3d.poke', { count: quality.poke_through_count })
                      : t('v3d.cleanFit')}
                  </span>
                </>
              )}
              {' · '}
              {(stats.motion_mm_s ?? 0) < 15 ? t('v3d.settled') : t('v3d.settling')}
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

export default Viewport3D;
