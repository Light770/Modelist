export interface OrbitCamera3D {
  yaw: number;
  pitch: number;
  dist: number;
  targetX: number;
  targetY: number;
  targetZ: number;
}

export type Vec3 = [number, number, number];

export const CAMERA_FOV_RAD = 0.62;
export const MIN_CAMERA_DIST = 0.06;
export const MAX_CAMERA_DIST = 14.0;

/**
 * Singularity-free spherical camera basis: valid for any yaw and pitch (including +-90 deg
 * poles and inverted upside-down orbits) without gimbal lock.
 */
export function cameraBasis(cam: OrbitCamera3D): {
  eye: Vec3;
  fwd: Vec3;
  right: Vec3;
  up: Vec3;
  fz: Vec3;
} {
  const cp = Math.cos(cam.pitch);
  const sp = Math.sin(cam.pitch);
  const sy = Math.sin(cam.yaw);
  const cy = Math.cos(cam.yaw);

  const fz: Vec3 = [cp * sy, sp, cp * cy];
  const right: Vec3 = [cy, 0, -sy];
  const up: Vec3 = [-sp * sy, cp, -sp * cy];
  const fwd: Vec3 = [-fz[0], -fz[1], -fz[2]];
  const eye: Vec3 = [
    cam.targetX + fz[0] * cam.dist,
    cam.targetY + fz[1] * cam.dist,
    cam.targetZ + fz[2] * cam.dist,
  ];
  return { eye, fwd, right, up, fz };
}

/**
 * Column-major OpenGL view matrix from the singularity-free camera basis.
 */
export function makeViewMatrix(cam: OrbitCamera3D): Float32Array {
  const { eye, right, up, fz } = cameraBasis(cam);
  return new Float32Array([
    right[0], up[0], fz[0], 0,
    right[1], up[1], fz[1], 0,
    right[2], up[2], fz[2], 0,
    -(right[0] * eye[0] + right[1] * eye[1] + right[2] * eye[2]),
    -(up[0] * eye[0] + up[1] * eye[1] + up[2] * eye[2]),
    -(fz[0] * eye[0] + fz[1] * eye[1] + fz[2] * eye[2]),
    1,
  ]);
}

export function cameraClipPlanes(dist: number): { near: number; far: number } {
  return {
    near: Math.max(0.005, dist * 0.01),
    far: Math.max(30.0, dist * 10.0),
  };
}

export function unprojectCameraRay(
  clientX: number,
  clientY: number,
  rect: { left: number; top: number; width: number; height: number },
  cam: OrbitCamera3D,
): { origin: Vec3; dir: Vec3; fwd: Vec3 } {
  const { eye, fwd, right, up } = cameraBasis(cam);
  const w = Math.max(1, rect.width);
  const h = Math.max(1, rect.height);
  const ndcX = ((clientX - rect.left) / w) * 2 - 1;
  const ndcY = 1 - ((clientY - rect.top) / h) * 2;
  const tanHalf = Math.tan(CAMERA_FOV_RAD * 0.5);
  const aspect = w / h;
  const dx = fwd[0] + right[0] * ndcX * tanHalf * aspect + up[0] * ndcY * tanHalf;
  const dy = fwd[1] + right[1] * ndcX * tanHalf * aspect + up[1] * ndcY * tanHalf;
  const dz = fwd[2] + right[2] * ndcX * tanHalf * aspect + up[2] * ndcY * tanHalf;
  const len = Math.hypot(dx, dy, dz) || 1;
  return { origin: eye, dir: [dx / len, dy / len, dz / len], fwd };
}

export function applyOrbitDelta(
  cam: OrbitCamera3D,
  dx: number,
  dy: number,
  yawSign: number,
): void {
  cam.yaw -= dx * 0.0085 * yawSign;
  cam.pitch += dy * 0.0075;
}

export function applyPanDelta(
  cam: OrbitCamera3D,
  dx: number,
  dy: number,
  viewportHeight: number,
): void {
  const { right, up } = cameraBasis(cam);
  const scale = (2 * cam.dist * Math.tan(CAMERA_FOV_RAD * 0.5)) / Math.max(1, viewportHeight);
  cam.targetX += (-right[0] * dx + up[0] * dy) * scale;
  cam.targetY += (-right[1] * dx + up[1] * dy) * scale;
  cam.targetZ += (-right[2] * dx + up[2] * dy) * scale;
}

export function applyZoomFactor(cam: OrbitCamera3D, factor: number): void {
  if (!Number.isFinite(factor) || factor <= 0) return;
  cam.dist = Math.max(MIN_CAMERA_DIST, Math.min(MAX_CAMERA_DIST, cam.dist / factor));
}

export function applyFreeMove(
  cam: OrbitCamera3D,
  input: { forward: number; right: number; up: number; sprint: boolean },
  dtSec: number,
): void {
  if (dtSec <= 0) return;
  const { fwd, right } = cameraBasis(cam);
  const speed = Math.max(0.25, cam.dist * 0.9) * (input.sprint ? 2.4 : 1.0) * dtSec;
  cam.targetX += (fwd[0] * input.forward + right[0] * input.right) * speed;
  cam.targetY += (fwd[1] * input.forward + right[1] * input.right + input.up) * speed;
  cam.targetZ += (fwd[2] * input.forward + right[2] * input.right) * speed;
}

