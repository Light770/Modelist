use crate::avatar::Vec3;
use std::collections::BTreeMap;

const LEAF_SIZE: usize = 6;

#[derive(Debug, Clone, Copy)]
pub enum TriFeature {
    Face,
    Edge(usize),
    Vert(usize),
}

#[derive(Debug, Clone)]
pub struct TriGeom {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub face_n: Vec3,
    pub edge_n: [Vec3; 3],
    pub vert_n: [Vec3; 3],
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub centroid: Vec3,
}

#[derive(Debug, Clone)]
struct BvhNode {
    min: Vec3,
    max: Vec3,
    /// When `count == 0`, `idx` is left child index (right child is `idx + 1`).
    /// When `count > 0`, leaf covers `tris[idx .. idx + count]`.
    idx: u32,
    count: u32,
}

/// Exact triangle-mesh BVH with Bærentzen–Aanæs angle-weighted pseudo-normals for watertight SDF queries.
#[derive(Debug, Clone)]
pub struct MeshSdfBvh {
    nodes: Vec<BvhNode>,
    tris: Vec<TriGeom>,
}

impl MeshSdfBvh {
    pub fn build(positions: &[f32], indices: &[u32]) -> Self {
        let n_verts = positions.len() / 3;
        let get_v = |vi: usize| -> Vec3 {
            Vec3::new(positions[vi * 3], positions[vi * 3 + 1], positions[vi * 3 + 2])
        };

        let mut vert_n_acc = vec![Vec3::ZERO; n_verts];
        let mut edge_n_acc: BTreeMap<(u32, u32), Vec3> = BTreeMap::new();
        let mut raw_tris = Vec::with_capacity(indices.len() / 3);

        for tri in indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0], tri[1], tri[2]);
            let v0 = get_v(i0 as usize);
            let v1 = get_v(i1 as usize);
            let v2 = get_v(i2 as usize);
            let e01 = v1.sub(v0);
            let e02 = v2.sub(v0);
            let cross = e01.cross(e02);
            let area2 = cross.len();
            if area2 < 1e-12 {
                continue;
            }
            let fn_u = cross.scale(1.0 / area2);

            // Accumulate undirected edge normals (01, 12, 20)
            for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
                let key = if a < b { (a, b) } else { (b, a) };
                let ent = edge_n_acc.entry(key).or_insert(Vec3::ZERO);
                *ent = ent.add(fn_u);
            }

            // Angle-weighted vertex normals
            let d01 = e01.normalized();
            let d12 = v2.sub(v1).normalized();
            let d20 = v0.sub(v2).normalized();
            let a0 = d01.dot(d20.scale(-1.0)).clamp(-1.0, 1.0).acos();
            let a1 = d01.scale(-1.0).dot(d12).clamp(-1.0, 1.0).acos();
            let a2 = d12.scale(-1.0).dot(d20).clamp(-1.0, 1.0).acos();
            vert_n_acc[i0 as usize] = vert_n_acc[i0 as usize].add(fn_u.scale(a0));
            vert_n_acc[i1 as usize] = vert_n_acc[i1 as usize].add(fn_u.scale(a1));
            vert_n_acc[i2 as usize] = vert_n_acc[i2 as usize].add(fn_u.scale(a2));

            raw_tris.push((i0, i1, i2, v0, v1, v2, fn_u));
        }

        for n in &mut vert_n_acc {
            *n = n.normalized();
        }
        for n in edge_n_acc.values_mut() {
            *n = n.normalized();
        }

        let mut tris: Vec<TriGeom> = raw_tris
            .into_iter()
            .map(|(i0, i1, i2, v0, v1, v2, face_n)| {
                let edge_n = [
                    edge_n_acc[&if i0 < i1 { (i0, i1) } else { (i1, i0) }],
                    edge_n_acc[&if i1 < i2 { (i1, i2) } else { (i2, i1) }],
                    edge_n_acc[&if i2 < i0 { (i2, i0) } else { (i0, i2) }],
                ];
                let vert_n = [
                    vert_n_acc[i0 as usize],
                    vert_n_acc[i1 as usize],
                    vert_n_acc[i2 as usize],
                ];
                let aabb_min = Vec3::new(
                    v0.x.min(v1.x).min(v2.x),
                    v0.y.min(v1.y).min(v2.y),
                    v0.z.min(v1.z).min(v2.z),
                );
                let aabb_max = Vec3::new(
                    v0.x.max(v1.x).max(v2.x),
                    v0.y.max(v1.y).max(v2.y),
                    v0.z.max(v1.z).max(v2.z),
                );
                let centroid = v0.add(v1).add(v2).scale(1.0 / 3.0);
                TriGeom {
                    v0,
                    v1,
                    v2,
                    face_n,
                    edge_n,
                    vert_n,
                    aabb_min,
                    aabb_max,
                    centroid,
                }
            })
            .collect();

        if tris.is_empty() {
            return Self {
                nodes: Vec::new(),
                tris,
            };
        }

        let mut nodes = Vec::with_capacity(tris.len() / LEAF_SIZE * 2 + 4);
        let n_tris = tris.len();
        build_bvh_recursive(&mut tris, 0, n_tris, &mut nodes);
        Self { nodes, tris }
    }

    pub fn signed_distance(&self, p: Vec3) -> (f32, Vec3) {
        self.signed_distance_within(p, f32::INFINITY)
            .unwrap_or((1.0, Vec3::new(0.0, 1.0, 0.0)))
    }

    /// Returns exact `(signed_distance_m, outward_unit_normal)` if surface is within `max_dist` meters.
    /// Iterative fixed-stack BVH traversal performs zero heap allocations per query.
    pub fn signed_distance_within(&self, p: Vec3, max_dist: f32) -> Option<(f32, Vec3)> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut best_d2 = if max_dist.is_finite() {
            max_dist * max_dist
        } else {
            f32::INFINITY
        };
        if aabb_dist_sq(p, self.nodes[0].min, self.nodes[0].max) > best_d2 {
            return None;
        }

        let mut best_q = Vec3::ZERO;
        let mut best_n_pseudo = Vec3::new(0.0, 1.0, 0.0);
        let mut found = false;

        let mut stack = [0u32; 64];
        let mut sp = 1usize;
        stack[0] = 0;

        while sp > 0 {
            sp -= 1;
            let node_idx = stack[sp] as usize;
            let node = &self.nodes[node_idx];
            if aabb_dist_sq(p, node.min, node.max) > best_d2 {
                continue;
            }
            if node.count > 0 {
                let start = node.idx as usize;
                let end = start + node.count as usize;
                for t in &self.tris[start..end] {
                    if aabb_dist_sq(p, t.aabb_min, t.aabb_max) > best_d2 {
                        continue;
                    }
                    let (q, feat) = closest_point_on_triangle(p, t.v0, t.v1, t.v2);
                    let d2 = p.sub(q).len_sq();
                    if !found || d2 < best_d2 {
                        best_d2 = d2;
                        best_q = q;
                        best_n_pseudo = match feat {
                            TriFeature::Face => t.face_n,
                            TriFeature::Edge(e) => t.edge_n[e],
                            TriFeature::Vert(v) => t.vert_n[v],
                        };
                        found = true;
                    }
                }
            } else {
                let left = node.idx as usize;
                let right = left + 1;
                let dl = aabb_dist_sq(p, self.nodes[left].min, self.nodes[left].max);
                let dr = aabb_dist_sq(p, self.nodes[right].min, self.nodes[right].max);
                // Push farther child first so closer child pops immediately next
                if dl < dr {
                    if dr <= best_d2 && sp < 63 {
                        stack[sp] = right as u32;
                        sp += 1;
                    }
                    if dl <= best_d2 && sp < 63 {
                        stack[sp] = left as u32;
                        sp += 1;
                    }
                } else {
                    if dl <= best_d2 && sp < 63 {
                        stack[sp] = left as u32;
                        sp += 1;
                    }
                    if dr <= best_d2 && sp < 63 {
                        stack[sp] = right as u32;
                        sp += 1;
                    }
                }
            }
        }

        if !found {
            return None;
        }
        let diff = p.sub(best_q);
        let dist = best_d2.sqrt();
        let sign = if diff.dot(best_n_pseudo) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let out_n = if dist > 1e-6 {
            diff.scale(sign / dist)
        } else {
            best_n_pseudo
        };
        Some((sign * dist, out_n))
    }
}

fn build_bvh_recursive(
    tris: &mut [TriGeom],
    start: usize,
    end: usize,
    nodes: &mut Vec<BvhNode>,
) -> u32 {
    let mut bmin = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut bmax = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut cmin = bmin;
    let mut cmax = bmax;
    for t in &tris[start..end] {
        bmin.x = bmin.x.min(t.aabb_min.x);
        bmin.y = bmin.y.min(t.aabb_min.y);
        bmin.z = bmin.z.min(t.aabb_min.z);
        bmax.x = bmax.x.max(t.aabb_max.x);
        bmax.y = bmax.y.max(t.aabb_max.y);
        bmax.z = bmax.z.max(t.aabb_max.z);
        cmin.x = cmin.x.min(t.centroid.x);
        cmin.y = cmin.y.min(t.centroid.y);
        cmin.z = cmin.z.min(t.centroid.z);
        cmax.x = cmax.x.max(t.centroid.x);
        cmax.y = cmax.y.max(t.centroid.y);
        cmax.z = cmax.z.max(t.centroid.z);
    }

    let node_idx = nodes.len() as u32;
    let count = end - start;
    if count <= LEAF_SIZE {
        nodes.push(BvhNode {
            min: bmin,
            max: bmax,
            idx: start as u32,
            count: count as u32,
        });
        return node_idx;
    }

    let ext = cmax.sub(cmin);
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    let mid = start + count / 2;
    tris[start..end].select_nth_unstable_by(mid - start, |a, b| {
        let va = match axis {
            0 => a.centroid.x,
            1 => a.centroid.y,
            _ => a.centroid.z,
        };
        let vb = match axis {
            0 => b.centroid.x,
            1 => b.centroid.y,
            _ => b.centroid.z,
        };
        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
    });

    nodes.push(BvhNode {
        min: bmin,
        max: bmax,
        idx: 0,
        count: 0,
    });
    // Left child sits immediately at `node_idx + 1`; reserve right child sequentially in BFS?
    // Store left child index explicitly after left subtree; allocate right child right next to left?
    // Wait: node layout expects right == left + 1 ONLY if both are pushed before recursing!
    // Pre-allocate BOTH child slots so left = child_base and right = child_base + 1!
    let child_base = nodes.len() as u32;
    nodes[node_idx as usize].idx = child_base;
    nodes.push(BvhNode { min: bmin, max: bmax, idx: 0, count: 0 });
    nodes.push(BvhNode { min: bmin, max: bmax, idx: 0, count: 0 });
    fill_bvh_slot(tris, start, mid, child_base as usize, nodes);
    fill_bvh_slot(tris, mid, end, (child_base + 1) as usize, nodes);
    node_idx
}

fn fill_bvh_slot(
    tris: &mut [TriGeom],
    start: usize,
    end: usize,
    slot: usize,
    nodes: &mut Vec<BvhNode>,
) {
    let mut bmin = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut bmax = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut cmin = bmin;
    let mut cmax = bmax;
    for t in &tris[start..end] {
        bmin.x = bmin.x.min(t.aabb_min.x);
        bmin.y = bmin.y.min(t.aabb_min.y);
        bmin.z = bmin.z.min(t.aabb_min.z);
        bmax.x = bmax.x.max(t.aabb_max.x);
        bmax.y = bmax.y.max(t.aabb_max.y);
        bmax.z = bmax.z.max(t.aabb_max.z);
        cmin.x = cmin.x.min(t.centroid.x);
        cmin.y = cmin.y.min(t.centroid.y);
        cmin.z = cmin.z.min(t.centroid.z);
        cmax.x = cmax.x.max(t.centroid.x);
        cmax.y = cmax.y.max(t.centroid.y);
        cmax.z = cmax.z.max(t.centroid.z);
    }

    let count = end - start;
    if count <= LEAF_SIZE {
        nodes[slot] = BvhNode {
            min: bmin,
            max: bmax,
            idx: start as u32,
            count: count as u32,
        };
        return;
    }

    let ext = cmax.sub(cmin);
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    let mid = start + count / 2;
    tris[start..end].select_nth_unstable_by(mid - start, |a, b| {
        let va = match axis {
            0 => a.centroid.x,
            1 => a.centroid.y,
            _ => a.centroid.z,
        };
        let vb = match axis {
            0 => b.centroid.x,
            1 => b.centroid.y,
            _ => b.centroid.z,
        };
        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let child_base = nodes.len() as u32;
    nodes[slot] = BvhNode {
        min: bmin,
        max: bmax,
        idx: child_base,
        count: 0,
    };
    nodes.push(BvhNode { min: bmin, max: bmax, idx: 0, count: 0 });
    nodes.push(BvhNode { min: bmin, max: bmax, idx: 0, count: 0 });
    fill_bvh_slot(tris, start, mid, child_base as usize, nodes);
    fill_bvh_slot(tris, mid, end, (child_base + 1) as usize, nodes);
}

#[inline]
fn aabb_dist_sq(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    let dx = if p.x < min.x {
        min.x - p.x
    } else if p.x > max.x {
        p.x - max.x
    } else {
        0.0
    };
    let dy = if p.y < min.y {
        min.y - p.y
    } else if p.y > max.y {
        p.y - max.y
    } else {
        0.0
    };
    let dz = if p.z < min.z {
        min.z - p.z
    } else if p.z > max.z {
        p.z - max.z
    } else {
        0.0
    };
    dx * dx + dy * dy + dz * dz
}

/// Exact Ericson Voronoi-region closest point + winning topological feature.
#[inline]
fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (Vec3, TriFeature) {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let ap = p.sub(a);
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, TriFeature::Vert(0));
    }

    let bp = p.sub(b);
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, TriFeature::Vert(1));
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (a.add(ab.scale(v)), TriFeature::Edge(0));
    }

    let cp = p.sub(c);
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, TriFeature::Vert(2));
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (a.add(ac.scale(w)), TriFeature::Edge(2));
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b.add(c.sub(b).scale(w)), TriFeature::Edge(1));
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (a.add(ab.scale(v)).add(ac.scale(w)), TriFeature::Face)
}
