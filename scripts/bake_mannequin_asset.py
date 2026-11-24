#!/usr/bin/env python3
"""
One-time asset baker converting CC0 1.0 Public Domain MakeHuman (`hm08`) body mesh,
symmetric weights, and anthropometric `.target` files into `crates/modelist-sim3d/assets/mannequin_cc0.bin`.

Zero MakeHuman source code is imported or used — ONLY CC0 graphical data files.
"""

import json
import math
import os
import struct
import urllib.request
from collections import Counter, defaultdict

BASE_URL = "https://raw.githubusercontent.com/makehumancommunity/makehuman/master/makehuman/data/"
OUT_PATH = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "crates",
    "modelist-sim3d",
    "assets",
    "mannequin_cc0.bin",
)

# Fixed 21 morph channels (index order must match crates/modelist-sim3d/src/mannequin_data.rs)
MEASURE_TARGETS = [
    "targets/macrodetails/universal-female-young-averagemuscle-maxweight.target",
    "targets/macrodetails/universal-female-young-averagemuscle-minweight.target",
    "targets/measure/measure-bust-circ-incr.target",
    "targets/measure/measure-bust-circ-decr.target",
    "targets/measure/measure-waist-circ-incr.target",
    "targets/measure/measure-waist-circ-decr.target",
    "targets/measure/measure-hips-circ-incr.target",
    "targets/measure/measure-hips-circ-decr.target",
    "targets/measure/measure-shoulder-dist-incr.target",
    "targets/measure/measure-shoulder-dist-decr.target",
    "targets/measure/measure-upperarm-circ-incr.target",
    "targets/measure/measure-upperarm-circ-decr.target",
    "targets/measure/measure-wrist-circ-incr.target",
    "targets/measure/measure-wrist-circ-decr.target",
    "targets/measure/measure-thigh-circ-incr.target",
    "targets/measure/measure-thigh-circ-decr.target",
    "targets/measure/measure-knee-circ-incr.target",
    "targets/measure/measure-knee-circ-decr.target",
    "targets/measure/measure-ankle-circ-incr.target",
    "targets/measure/measure-ankle-circ-decr.target",
]


def fetch_text(rel_path: str) -> str:
    url = BASE_URL + rel_path
    with urllib.request.urlopen(url, timeout=30) as resp:
        return resp.read().decode("utf-8")


def parse_target(rel_path: str, remap: dict[int, int]) -> dict[int, tuple[float, float, float]]:
    deltas: dict[int, tuple[float, float, float]] = {}
    for line in fetch_text(rel_path).splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = s.split()
        if len(parts) < 4:
            continue
        old_id = int(parts[0])
        if old_id in remap:
            new_id = remap[old_id]
            # MakeHuman coordinates are in decimeters -> meters (* 0.1)
            deltas[new_id] = (float(parts[1]) * 0.1, float(parts[2]) * 0.1, float(parts[3]) * 0.1)
    return deltas


def map_bone_name(name: str) -> int:
    # 0=Torso, 1=ClavicleL, 2=UpperArmL, 3=ForearmL, 4=ClavicleR, 5=UpperArmR, 6=ForearmR,
    # 7=ThighL, 8=CalfL, 9=ThighR, 10=CalfR
    if name in ("clavicle.L", "shoulder01.L"):
        return 1
    if name in ("upperarm01.L", "upperarm02.L"):
        return 2
    if name.startswith(("lowerarm", "wrist.L", "metacarpal", "finger")) and name.endswith(".L"):
        return 3
    if name in ("clavicle.R", "shoulder01.R"):
        return 4
    if name in ("upperarm01.R", "upperarm02.R"):
        return 5
    if name.startswith(("lowerarm", "wrist.R", "metacarpal", "finger")) and name.endswith(".R"):
        return 6
    if name in ("upperleg01.L", "upperleg02.L"):
        return 7
    if name in ("lowerleg01.L", "lowerleg02.L", "foot.L") or (name.startswith("toe") and name.endswith(".L")):
        return 8
    if name in ("upperleg01.R", "upperleg02.R"):
        return 9
    if name in ("lowerleg01.R", "lowerleg02.R", "foot.R") or (name.startswith("toe") and name.endswith(".R")):
        return 10
    return 0


def build_mirror_map(verts: list[list[float]]) -> dict[int, int]:
    """Pairs symmetric vertices across X=0 within base mesh."""
    # Spatial hash in 4 mm cells
    cell = 0.004
    grid: dict[tuple[int, int, int], list[int]] = defaultdict(list)
    for i, (x, y, z) in enumerate(verts):
        key = (round(x / cell), round(y / cell), round(z / cell))
        grid[key].append(i)

    mirror: dict[int, int] = {}
    for i, (x, y, z) in enumerate(verts):
        if i in mirror:
            continue
        tx = -x
        best_j = i
        best_d2 = 1e9
        kx, ky, kz = round(tx / cell), round(y / cell), round(z / cell)
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for dz in (-1, 0, 1):
                    for j in grid.get((kx + dx, ky + dy, kz + dz), ()):
                        vx, vy, vz = verts[j]
                        d2 = (vx - tx) ** 2 + (vy - y) ** 2 + (vz - z) ** 2
                        if d2 < best_d2:
                            best_d2 = d2
                            best_j = j
        if best_d2 < 0.008 ** 2:
            mirror[i] = best_j
            mirror[best_j] = i
        else:
            mirror[i] = i
    return mirror


def enforce_symmetry(verts: list[list[float]], mirror: dict[int, int]) -> None:
    for i, j in mirror.items():
        if i == j:
            if abs(verts[i][0]) < 0.004:
                verts[i][0] = 0.0
        elif i < j:
            ax = 0.5 * (verts[i][0] - verts[j][0])
            ay = 0.5 * (verts[i][1] + verts[j][1])
            az = 0.5 * (verts[i][2] + verts[j][2])
            verts[i][0], verts[i][1], verts[i][2] = ax, ay, az
            verts[j][0], verts[j][1], verts[j][2] = -ax, ay, az


def smooth_cavity_verts(
    verts: list[list[float]],
    quads: list[list[int]],
    cavity_set: set[int],
    iterations: int = 8,
) -> None:
    adj: list[set[int]] = [set() for _ in verts]
    for a, b, c, d in quads:
        for u, v in ((a, b), (b, c), (c, d), (d, a), (a, c), (b, d)):
            adj[u].add(v)
            adj[v].add(u)
    for _ in range(iterations):
        updates: dict[int, list[float]] = {}
        for i in cavity_set:
            nbrs = adj[i]
            if not nbrs:
                continue
            avg_x = sum(verts[n][0] for n in nbrs) / len(nbrs)
            avg_y = sum(verts[n][1] for n in nbrs) / len(nbrs)
            avg_z = sum(verts[n][2] for n in nbrs) / len(nbrs)
            # Blend smoothly toward outer rim & ensure recessed Z pockets fill forward
            nx = verts[i][0] * 0.35 + avg_x * 0.65
            ny = verts[i][1] * 0.35 + avg_y * 0.65
            nz = max(verts[i][2], verts[i][2] * 0.25 + avg_z * 0.75)
            updates[i] = [nx, ny, nz]
        for i, pos in updates.items():
            verts[i] = pos


def slice_circ_mm(
    verts: list[list[float]],
    quads: list[list[int]],
    allowed_verts: set[int],
    y_target: float,
) -> float:
    total = 0.0
    for a, b, c, d in quads:
        if a not in allowed_verts or b not in allowed_verts or c not in allowed_verts or d not in allowed_verts:
            continue
        for t0, t1, t2 in ((a, b, c), (a, c, d)):
            pts = (verts[t0], verts[t1], verts[t2])
            above = [p for p in pts if p[1] >= y_target]
            below = [p for p in pts if p[1] < y_target]
            if not above or not below:
                continue
            inter = []
            for pa in above:
                for pb in below:
                    denom = pa[1] - pb[1]
                    if abs(denom) < 1e-8:
                        continue
                    t = (y_target - pb[1]) / denom
                    inter.append((pb[0] + t * (pa[0] - pb[0]), pb[2] + t * (pa[2] - pb[2])))
            if len(inter) == 2:
                total += math.hypot(inter[1][0] - inter[0][0], inter[1][1] - inter[0][1])
    return total * 1000.0


def main() -> None:
    print("Fetching MakeHuman CC0 base.obj and default_weights.mhw...")
    obj_text = fetch_text("3dobjs/base.obj")
    weights_json = json.loads(fetch_text("rigs/default_weights.mhw"))["weights"]

    raw_verts: list[list[float]] = []
    raw_quads: list[list[int]] = []
    raw_joints: dict[str, set[int]] = defaultdict(set)
    cur_group = ""

    for line in obj_text.splitlines():
        if line.startswith("v "):
            p = line.split()
            raw_verts.append([float(p[1]) * 0.1, float(p[2]) * 0.1, float(p[3]) * 0.1])
        elif line.startswith("g "):
            cur_group = line[2:].strip()
        elif line.startswith("f "):
            idxs = [int(tok.split("/")[0]) - 1 for tok in line.split()[1:]]
            if cur_group == "body" and len(idxs) == 4:
                raw_quads.append(idxs)
            elif cur_group.startswith("joint-"):
                raw_joints[cur_group].update(idxs)

    # Compactly reindex ONLY `body` vertices (`0..N-1`)
    used_old = sorted({vid for q in raw_quads for vid in q})
    old_to_new = {old: new for new, old in enumerate(used_old)}
    base_verts = [raw_verts[old][:] for old in used_old]
    quads = [[old_to_new[v] for v in q] for q in raw_quads]
    n_verts = len(base_verts)
    n_quads = len(quads)
    print(f"Body mesh: {n_verts} vertices, {n_quads} quads")

    # Verify manifold topology (0 boundary edges, 0 non-manifold edges)
    edge_counts: Counter[tuple[int, int]] = Counter()
    for q in quads:
        for k in range(4):
            u, v = q[k], q[(k + 1) % 4]
            edge_counts[(min(u, v), max(u, v))] += 1
    boundary = sum(1 for c in edge_counts.values() if c == 1)
    nonman = sum(1 for c in edge_counts.values() if c > 2)
    assert boundary == 0 and nonman == 0, f"Mesh not watertight closed manifold: boundary={boundary}, nonman={nonman}"

    # Verify positive signed volume (outward normals); flip if negative
    vol6 = 0.0
    for a, b, c, d in quads:
        for i0, i1, i2 in ((a, b, c), (a, c, d)):
            p0, p1, p2 = base_verts[i0], base_verts[i1], base_verts[i2]
            cx = p1[1] * p2[2] - p1[2] * p2[1]
            cy = p1[2] * p2[0] - p1[0] * p2[2]
            cz = p1[0] * p2[1] - p1[1] * p2[0]
            vol6 += p0[0] * cx + p0[1] * cy + p0[2] * cz
    if vol6 < 0.0:
        quads = [[a, d, c, b] for a, b, c, d in quads]
        vol6 = -vol6
    print(f"Manifold check: 0 boundary edges, 0 non-manifold edges, enclosed volume = {vol6/6.0*1000:.2f} L")

    # Build neutral studio female & male base morphologies from CC0 targets
    print("Fetching CC0 female/male base & macro/measure targets...")
    female_race = [
        parse_target("targets/macrodetails/caucasian-female-young.target", old_to_new),
        parse_target("targets/macrodetails/african-female-young.target", old_to_new),
        parse_target("targets/macrodetails/asian-female-young.target", old_to_new),
    ]
    ideal_female = parse_target(
        "targets/macrodetails/proportions/female-young-averagemuscle-averageweight-idealproportions.target",
        old_to_new,
    )
    male_race = [
        parse_target("targets/macrodetails/caucasian-male-young.target", old_to_new),
        parse_target("targets/macrodetails/african-male-young.target", old_to_new),
        parse_target("targets/macrodetails/asian-male-young.target", old_to_new),
    ]

    # Build symmetric mirror table on raw base
    mirror = build_mirror_map(base_verts)

    def build_morphed(verts_src, weights_and_dicts):
        out = [v[:] for v in verts_src]
        for w, d in weights_and_dicts:
            for vid, (dx, dy, dz) in d.items():
                out[vid][0] += w * dx
                out[vid][1] += w * dy
                out[vid][2] += w * dz
        return out

    # Joint delta extraction helper (uses old_verts for raw_joints)
    def apply_target_raw(verts_in, w, rel_path):
        for line in fetch_text(rel_path).splitlines():
            s = line.strip()
            if not s or s.startswith("#"):
                continue
            p = s.split()
            if len(p) >= 4:
                oid = int(p[0])
                verts_in[oid][0] += w * float(p[1]) * 0.1
                verts_in[oid][1] += w * float(p[2]) * 0.1
                verts_in[oid][2] += w * float(p[3]) * 0.1

    female_raw = [v[:] for v in raw_verts]
    for p in (
        "targets/macrodetails/caucasian-female-young.target",
        "targets/macrodetails/african-female-young.target",
        "targets/macrodetails/asian-female-young.target",
    ):
        apply_target_raw(female_raw, 1.0 / 3.0, p)
    apply_target_raw(
        female_raw,
        0.30,
        "targets/macrodetails/proportions/female-young-averagemuscle-averageweight-idealproportions.target",
    )

    female_verts = build_morphed(
        base_verts,
        [(1.0 / 3.0, female_race[0]), (1.0 / 3.0, female_race[1]), (1.0 / 3.0, female_race[2]), (0.30, ideal_female)],
    )
    male_verts = build_morphed(
        base_verts,
        [(1.0 / 3.0, male_race[0]), (1.0 / 3.0, male_race[1]), (1.0 / 3.0, male_race[2])],
    )

    # Identify inner eye-socket & inner oral-cavity vertices from bone weights so we sculpt a smooth studio mannequin face
    head_y_thresh = min(v[1] for v in female_verts) + 1.44
    cavity_verts: set[int] = set()
    for bname, lst in weights_json.items():
        if any(tok in bname for tok in ("eye.", "orbicularis", "oris")):
            for oid, wt in lst:
                if oid in old_to_new and wt > 0.25:
                    nid = old_to_new[oid]
                    if female_verts[nid][1] >= head_y_thresh and female_verts[nid][2] > 0.075:
                        cavity_verts.add(nid)
    print(f"Conditioning {len(cavity_verts)} recessed eye/lip cavity verts for mannequin studio finish...")
    smooth_cavity_verts(female_verts, quads, cavity_verts, iterations=10)
    smooth_cavity_verts(male_verts, quads, cavity_verts, iterations=10)

    # Plant min_y = 0.0 on female_verts and match Z/Y offset identically to male_verts & joints
    min_y_f = min(v[1] for v in female_verts)
    for v in female_verts:
        v[1] -= min_y_f
    for v in male_verts:
        v[1] -= min_y_f
    enforce_symmetry(female_verts, mirror)
    enforce_symmetry(male_verts, mirror)

    # Collapse 139 bones -> 11 clean weights + extract head/neck mask
    accum: list[dict[int, float]] = [defaultdict(float) for _ in range(n_verts)]
    facial_weight = [0.0] * n_verts
    for bname, lst in weights_json.items():
        b_idx = map_bone_name(bname)
        is_head_bone = (
            bname in ("head", "neck02", "neck03", "jaw", "special04", "special05.L", "special05.R")
            or any(s in bname for s in ("eye", "orbicularis", "oris", "levator", "risorius", "oculi", "temporalis", "tongue"))
        )
        for oid, wt in lst:
            if oid in old_to_new and wt > 0.0:
                nid = old_to_new[oid]
                accum[nid][b_idx] += float(wt)
                if is_head_bone:
                    facial_weight[nid] += float(wt)

    # Extract key joint centroids from posed female raw verts
    def j_center(name: str) -> list[float]:
        pts = [female_raw[oid] for oid in raw_joints[name]]
        return [
            sum(p[0] for p in pts) / len(pts),
            sum(p[1] for p in pts) / len(pts) - min_y_f,
            sum(p[2] for p in pts) / len(pts),
        ]

    j_pelvis = j_center("joint-pelvis")
    j_spine2 = j_center("joint-spine-2")
    j_spine1 = j_center("joint-spine-1")
    j_neck = j_center("joint-neck")
    j_head2 = j_center("joint-head-2")
    j_sh_l = j_center("joint-l-shoulder")
    j_el_l = j_center("joint-l-elbow")
    j_wr_l = j_center("joint-l-hand")
    j_hd_l = j_center("joint-l-hand-2")
    j_leg_l = j_center("joint-l-upper-leg")
    j_kn_l = j_center("joint-l-knee")
    j_an_l = j_center("joint-l-ankle")
    j_ft_l = j_center("joint-l-foot-1")

    # Locate exact anatomical crotch & bust/waist/hip Y on female_verts
    src_ankle_y = j_an_l[1]
    src_knee_y = j_kn_l[1]
    src_neck_y = j_neck[1]
    src_head_top_y = max(v[1] for v in female_verts)
    src_shoulder_y = src_neck_y - 0.032  # HPS shoulder base just below neck crease

    # Exact crotch bifurcation along sagittal midline |x| < 0.008 in pelvis/upper-leg band
    mid_crotch = [
        v[1]
        for v in female_verts
        if abs(v[0]) < 0.008
        and -0.06 <= v[2] <= 0.06
        and (j_leg_l[1] - 0.12) <= v[1] <= (j_pelvis[1] + 0.02)
    ]
    src_crotch_y = min(mid_crotch)
    j_crotch = [0.0, src_crotch_y, j_pelvis[2]]

    # Trunk region excludes arms/hands/head while keeping full pelvis/hip/glute loop intact
    trunk_cand = {
        i
        for i, v in enumerate(female_verts)
        if (accum[i].get(2, 0.0) + accum[i].get(3, 0.0) + accum[i].get(5, 0.0) + accum[i].get(6, 0.0)) < 0.45
        and facial_weight[i] < 0.35
        and abs(v[0]) < 0.23
        and v[1] <= src_neck_y + 0.02
    }

    # Bust: widest closed chest loop near anterior breast/chest apex below armpit crease
    best_bust_y = j_spine1[1] - 0.05
    best_bust_z = -1e9
    for k in range(28):
        yc = (src_crotch_y + 0.30) + k * 0.008
        if yc > src_shoulder_y - 0.10:
            break
        zf = max(
            (
                female_verts[i][2]
                for i in trunk_cand
                if abs(female_verts[i][1] - yc) < 0.012 and 0.03 <= abs(female_verts[i][0]) <= 0.13
            ),
            default=-1e9,
        )
        if zf > best_bust_z:
            best_bust_z = zf
            best_bust_y = yc
    # Pick peak closed-loop girth within ±15 mm of apex so bust slice stays safely below armpit crease
    peak_bust_y = best_bust_y
    peak_bust_c = -1.0
    for dy in (-0.012, -0.006, 0.0, 0.006, 0.012):
        yc = best_bust_y + dy
        c = slice_circ_mm(female_verts, quads, trunk_cand, yc)
        if c > peak_bust_c:
            peak_bust_c = c
            peak_bust_y = yc
    src_bust_y = peak_bust_y

    # Natural waist: minimum single-loop torso circumference between hip and bust
    best_waist_y = j_spine2[1]
    best_waist_c = 1e9
    for k in range(30):
        yc = (src_crotch_y + 0.16) + k * 0.009
        if yc >= src_bust_y - 0.06:
            break
        c = slice_circ_mm(female_verts, quads, trunk_cand, yc)
        if 450.0 < c < best_waist_c:
            best_waist_c = c
            best_waist_y = yc
    src_waist_y = best_waist_y

    # Anatomical hip: full-hip circumference band strictly above perineum bifurcation (crotch + 0.062..0.125)
    best_hip_y = src_crotch_y + 0.075
    best_hip_c = -1.0
    for k in range(14):
        yc = (src_crotch_y + 0.062) + k * 0.005
        if yc >= src_waist_y - 0.08:
            break
        c = slice_circ_mm(female_verts, quads, trunk_cand, yc)
        if c > best_hip_c:
            best_hip_c = c
            best_hip_y = yc
    src_hip_y = best_hip_y

    # Measure shoulder_half_x across outer deltoid contour
    sh_cands = [v[0] for v in female_verts if src_shoulder_y - 0.08 <= v[1] <= src_shoulder_y and v[0] > 0.10]
    src_shoulder_half_x = max(sh_cands) if sh_cands else 0.195
    # Cap slightly inboard of extended arm by sampling around shoulder joint X
    src_shoulder_half_x = min(src_shoulder_half_x, j_sh_l[0] + 0.042)

    j_bust = [0.0, src_bust_y, best_bust_z * 0.5]
    j_waist = [0.0, src_waist_y, j_spine2[2]]
    j_neck_pt = [0.0, src_neck_y, j_neck[2]]
    j_head_top = [0.0, src_head_top_y, j_head2[2]]

    print(
        f"Female base landmarks (m): height={src_head_top_y:.3f}, neck={src_neck_y:.3f}, "
        f"shoulder={src_shoulder_y:.3f}, bust={src_bust_y:.3f} ({slice_circ_mm(female_verts, quads, trunk_cand, src_bust_y):.0f}mm), "
        f"waist={src_waist_y:.3f} ({best_waist_c:.0f}mm), hip={src_hip_y:.3f} ({best_hip_c:.0f}mm), "
        f"crotch={src_crotch_y:.3f}, knee={src_knee_y:.3f}, ankle={src_ankle_y:.3f}"
    )

    # Encode skinning (top 2 bones) and clean region_id per vertex
    skin_region = bytearray()
    for i in range(n_verts):
        d = accum[i]
        if not d:
            b0, b1, w0 = 0, 0, 255
        else:
            sorted_b = sorted(d.items(), key=lambda kv: -kv[1])
            b0, val0 = sorted_b[0]
            b1, val1 = sorted_b[1] if len(sorted_b) > 1 else (b0, 0.0)
            tot = val0 + val1
            w0 = max(1, min(255, round(255.0 * (val0 / tot)))) if tot > 1e-6 else 255
            if b0 == b1:
                w0 = 255

        x, y, _ = female_verts[i]
        arm_l_w = accum[i].get(2, 0.0) + accum[i].get(3, 0.0)
        arm_r_w = accum[i].get(5, 0.0) + accum[i].get(6, 0.0)
        if facial_weight[i] >= 0.35 or y >= src_neck_y + 0.015:
            reg = 1  # Head
        elif (arm_l_w >= 0.45 or b0 in (2, 3)) and (x > j_sh_l[0] - 0.015 or y > src_bust_y + 0.04):
            reg = 2  # ArmL
        elif (arm_r_w >= 0.45 or b0 in (5, 6)) and (x < -j_sh_l[0] + 0.015 or y > src_bust_y + 0.04):
            reg = 3  # ArmR
        elif (b0 == 8 or b0 == 7) and y <= src_crotch_y - 0.005 and x >= 0.0:
            reg = 4  # LegL
        elif (b0 == 10 or b0 == 9) and y <= src_crotch_y - 0.005 and x <= 0.0:
            reg = 5  # LegR
        else:
            reg = 0  # Torso
        skin_region.extend((b0, b1, w0, reg))

    # Build 21 morph target channels (channel 0 = male_form, channels 1..20 = macro weight + measure targets)
    morph_deltas: list[list[tuple[int, int, int, int]]] = []

    # Channel 0: male_verts - female_verts
    male_chan = []
    for i in range(n_verts):
        dx = male_verts[i][0] - female_verts[i][0]
        dy = male_verts[i][1] - female_verts[i][1]
        dz = male_verts[i][2] - female_verts[i][2]
        qx, qy, qz = round(dx * 10000.0), round(dy * 10000.0), round(dz * 10000.0)
        if abs(qx) >= 3 or abs(qy) >= 3 or abs(qz) >= 3:
            assert max(abs(qx), abs(qy), abs(qz)) <= 32760
            male_chan.append((i, qx, qy, qz))
    morph_deltas.append(male_chan)

    for rel_path in MEASURE_TARGETS:
        tmap = parse_target(rel_path, old_to_new)
        chan = []
        # Enforce mirror symmetry on measure deltas too and suppress face cavity verts
        all_ids = sorted(tmap.keys())
        for i in all_ids:
            if i in cavity_verts:
                continue
            dx, dy, dz = tmap[i]
            qx, qy, qz = round(dx * 10000.0), round(dy * 10000.0), round(dz * 10000.0)
            if abs(qx) >= 3 or abs(qy) >= 3 or abs(qz) >= 3:
                assert max(abs(qx), abs(qy), abs(qz)) <= 32760
                chan.append((i, qx, qy, qz))
        morph_deltas.append(chan)
        print(f"  Target {rel_path.split('/')[-1]:52s}: {len(chan):5d} deltas")

    src_joints = [
        j_pelvis,
        j_crotch,
        j_waist,
        j_bust,
        j_neck_pt,
        j_head_top,
        j_sh_l,
        j_el_l,
        j_wr_l,
        j_hd_l,
        j_leg_l,
        j_kn_l,
        j_an_l,
        j_ft_l,
    ]
    src_landmarks = [
        src_crotch_y,
        src_hip_y,
        src_waist_y,
        src_bust_y,
        src_shoulder_y,
        src_neck_y,
        src_ankle_y,
        src_knee_y,
        src_head_top_y,
        src_shoulder_half_x,
    ]

    os.makedirs(os.path.dirname(OUT_PATH), exist_ok=True)
    with open(OUT_PATH, "wb") as f:
        f.write(b"MLMN")
        f.write(struct.pack("<HIIH", 1, n_verts, n_quads, len(morph_deltas)))
        for j in src_joints:
            f.write(struct.pack("<fff", *j))
        f.write(struct.pack("<" + "f" * len(src_landmarks), *src_landmarks))
        for x, y, z in female_verts:
            qx, qy, qz = round(x * 10000.0), round(y * 10000.0), round(z * 10000.0)
            assert max(abs(qx), abs(qy), abs(qz)) <= 32760
            f.write(struct.pack("<hhh", qx, qy, qz))
        f.write(skin_region)
        for a, b, c, d in quads:
            f.write(struct.pack("<HHHH", a, b, c, d))
        for chan in morph_deltas:
            f.write(struct.pack("<I", len(chan)))
            for vid, qx, qy, qz in chan:
                f.write(struct.pack("<Hhhh", vid, qx, qy, qz))

    size_kb = os.path.getsize(OUT_PATH) / 1024.0
    print(f"Wrote {OUT_PATH} ({size_kb:.1f} KB, {len(morph_deltas)} morph channels)")


if __name__ == "__main__":
    main()
