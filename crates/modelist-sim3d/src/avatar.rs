use crate::bvh::MeshSdfBvh;
use crate::mannequin_data::{
    get_template, BONE_CALF_L, BONE_CALF_R, BONE_FOREARM_L, BONE_FOREARM_R, BONE_THIGH_L,
    BONE_THIGH_R, BONE_UPPER_ARM_L, BONE_UPPER_ARM_R, J_ANKLE_L, J_BUST, J_CROTCH, J_ELBOW_L,
    J_HEAD_TOP, J_HIP_L, J_NECK_BASE, J_PELVIS, J_SHOULDER_L, J_WAIST, J_WRIST_L,
    MORPH_ANKLE_DECR, MORPH_ANKLE_INCR, MORPH_BUST_DECR, MORPH_BUST_INCR, MORPH_HIPS_DECR,
    MORPH_HIPS_INCR, MORPH_KNEE_DECR, MORPH_KNEE_INCR, MORPH_MALE_FORM, MORPH_SHOULDER_DECR,
    MORPH_SHOULDER_INCR, MORPH_THIGH_DECR, MORPH_THIGH_INCR, MORPH_UPPERARM_DECR,
    MORPH_UPPERARM_INCR, MORPH_WAIST_DECR, MORPH_WAIST_INCR, MORPH_WEIGHT_DECR,
    MORPH_WEIGHT_INCR, MORPH_WRIST_DECR, MORPH_WRIST_INCR, MorphDelta, REG_ARM_L, REG_LEG_L,
    REG_LEG_R, REG_TORSO,
};
use modelist_core::model::BodyMeasurements;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarPose {
    APose,
    ArmsRaised,
    TPose,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarBodyForm {
    #[default]
    Female,
    Neutral,
    Male,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    #[inline]
    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    #[inline]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline]
    pub fn len_sq(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn len(self) -> f32 {
        self.len_sq().sqrt()
    }

    #[inline]
    pub fn normalized(self) -> Self {
        let l = self.len();
        if l < 1e-7 {
            Self::new(0.0, 1.0, 0.0)
        } else {
            self.scale(1.0 / l)
        }
    }
}

#[derive(Debug, Clone)]
pub struct AvatarMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub torso_mask: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AvatarMeasuredMetrics {
    pub height_mm: f32,
    pub bust_mm: f32,
    pub waist_mm: f32,
    pub hip_mm: f32,
    pub shoulder_mm: f32,
    pub bicep_mm: f32,
    pub thigh_mm: f32,
}

const TORSO_RINGS: usize = 48;
const TORSO_SEGS: usize = 36;
const LIMB_BINS: usize = 20;

/// Watertight, anatomically sculpted CC0 MakeHuman (`hm08`) parametric dress-form mannequin
/// paired with an exact angle-weighted pseudo-normal triangle BVH SDF collider and closed-loop
/// millimeter circumference calibration. Feet rest at y = 0.0 m; +Y up, +Z front, +X wearer's left.
#[derive(Debug, Clone)]
pub struct ParametricAvatar {
    pub pose: AvatarPose,
    pub body_form: AvatarBodyForm,
    pub ankle_y: f32,
    pub knee_y: f32,
    pub crotch_y: f32,
    pub hip_y: f32,
    pub waist_y: f32,
    pub bust_y: f32,
    pub acromion_y: f32,
    pub shoulder_y: f32,
    pub neck_y: f32,
    pub head_y: f32,
    pub waist_rx: f32,
    pub waist_rz: f32,
    pub bust_rx: f32,
    pub bust_rz: f32,
    pub hip_rx: f32,
    pub hip_rz: f32,
    pub shoulder_half_x: f32,
    pub bicep_r: f32,
    pub wrist_r: f32,
    pub sleeve_len: f32,
    pub thigh_r: f32,
    pub knee_r: f32,
    pub ankle_r: f32,
    pub measured: AvatarMeasuredMetrics,
    pub render_mesh: AvatarMesh,
    bvh: MeshSdfBvh,
    torso_surface_rings: Vec<[Vec3; TORSO_SEGS]>,
    torso_ring_profile: Vec<(f32, f32, f32)>,
    arm_seg_l: (Vec3, Vec3),
    arm_seg_r: (Vec3, Vec3),
    arm_radii: Vec<f32>,
    leg_seg_l: (Vec3, Vec3),
    leg_seg_r: (Vec3, Vec3),
    leg_radii: Vec<f32>,
}

impl ParametricAvatar {
    pub fn from_measurements(m: &BodyMeasurements, pose: AvatarPose) -> Self {
        Self::from_measurements_with_form(m, pose, AvatarBodyForm::Female)
    }

    pub fn from_measurements_with_form(
        m: &BodyMeasurements,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        let tmpl = get_template();

        // Target landmark heights calibrated directly to BodyMeasurements (meters)
        let ankle_y = 0.080_f32;
        let inseam = (m.inseam_mm as f32) * 0.001;
        let crotch_y = ankle_y + (inseam * 0.995).clamp(0.60, 0.92);
        let knee_y = ankle_y + (crotch_y - ankle_y) * 0.53;
        let rise = (m.crotch_depth_mm as f32) * 0.001;
        let waist_y = crotch_y + rise.clamp(0.20, 0.34);
        let hip_y = (waist_y - (m.waist_to_hip_mm as f32) * 0.001).max(crotch_y + 0.050);

        let bodice_h = (m.back_waist_len_mm as f32) * 0.001;
        let arm_drop = (m.armscye_depth_mm as f32) * 0.001;
        let shoulder_y = waist_y + bodice_h.clamp(0.35, 0.52);
        let acromion_y = shoulder_y - 0.030;
        let bust_y = shoulder_y - arm_drop * 0.88;
        let neck_y = shoulder_y + 0.068;
        let head_y = neck_y + 0.090;
        let head_top_y = neck_y + 0.182;

        let (bust_rx, bust_rz) = ellipse_radii_from_circ((m.bust_mm as f32) * 0.001, 1.26);
        let (waist_rx, waist_rz) = ellipse_radii_from_circ((m.waist_mm as f32) * 0.001, 1.32);
        let (hip_rx, hip_rz) = ellipse_radii_from_circ((m.hip_mm as f32) * 0.001, 1.32);
        let shoulder_half_x = ((m.shoulder_width_mm as f32) * 0.0005).max(bust_rx + 0.018);
        let bicep_r = ((m.bicep_mm as f32) * 0.001 / std::f32::consts::TAU).clamp(0.034, 0.068);
        let wrist_r = ((m.wrist_mm as f32) * 0.001 / std::f32::consts::TAU).clamp(0.021, 0.040);
        let thigh_r = ((m.thigh_mm as f32) * 0.001 / std::f32::consts::TAU).clamp(0.066, 0.120);
        let knee_r = ((m.knee_mm as f32) * 0.001 / std::f32::consts::TAU).clamp(0.044, 0.080);
        let ankle_r = ((m.ankle_mm as f32) * 0.001 / std::f32::consts::TAU).clamp(0.030, 0.055);
        let sleeve_len = (m.sleeve_len_mm as f32) * 0.001;

        // Track anchor vertex rings near source joints so Stage B pivots adapt cleanly via morph deltas
        let anchor_sets: Vec<Vec<usize>> = tmpl
            .src_joints
            .iter()
            .map(|&jp| find_nearest_verts(&tmpl.base_verts, jp, 14))
            .collect();
        let base_anchor_centers: Vec<Vec3> = anchor_sets
            .iter()
            .map(|ids| avg_verts(&tmpl.base_verts, ids))
            .collect();

        // === STAGE A: Anthropometric CC0 Morph Blending ===
        let mut verts = tmpl.base_verts.clone();
        let male_w = match body_form {
            AvatarBodyForm::Female => 0.0_f32,
            AvatarBodyForm::Neutral => 0.5_f32,
            AvatarBodyForm::Male => 1.0_f32,
        };
        if male_w > 0.0 {
            apply_morph(&mut verts, &tmpl.morphs[MORPH_MALE_FORM], male_w);
        }

        // Gentle overall body mass pre-shaping + regional CC0 targets (bounded to preserve clean folds)
        let girth_delta = ((m.bust_mm as f32 - 865.0) / 180.0
            + (m.waist_mm as f32 - 646.0) / 160.0
            + (m.hip_mm as f32 - 974.0) / 180.0)
            / 3.0;
        if girth_delta > 0.0 {
            apply_morph(
                &mut verts,
                &tmpl.morphs[MORPH_WEIGHT_INCR],
                (girth_delta * 0.65).clamp(0.0, 0.85),
            );
        } else {
            apply_morph(
                &mut verts,
                &tmpl.morphs[MORPH_WEIGHT_DECR],
                ((-girth_delta) * 0.65).clamp(0.0, 0.75),
            );
        }

        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_BUST_INCR],
            &tmpl.morphs[MORPH_BUST_DECR],
            (m.bust_mm as f32 - 865.0) / 150.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_WAIST_INCR],
            &tmpl.morphs[MORPH_WAIST_DECR],
            (m.waist_mm as f32 - 646.0) / 150.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_HIPS_INCR],
            &tmpl.morphs[MORPH_HIPS_DECR],
            (m.hip_mm as f32 - 974.0) / 150.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_SHOULDER_INCR],
            &tmpl.morphs[MORPH_SHOULDER_DECR],
            (m.shoulder_width_mm as f32 - 385.0) / 65.0,
            0.80,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_UPPERARM_INCR],
            &tmpl.morphs[MORPH_UPPERARM_DECR],
            (m.bicep_mm as f32 - 280.0) / 80.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_WRIST_INCR],
            &tmpl.morphs[MORPH_WRIST_DECR],
            (m.wrist_mm as f32 - 160.0) / 45.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_THIGH_INCR],
            &tmpl.morphs[MORPH_THIGH_DECR],
            (m.thigh_mm as f32 - 540.0) / 110.0,
            0.85,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_KNEE_INCR],
            &tmpl.morphs[MORPH_KNEE_DECR],
            (m.knee_mm as f32 - 360.0) / 70.0,
            0.80,
        );
        apply_signed_morph_pair(
            &mut verts,
            &tmpl.morphs[MORPH_ANKLE_INCR],
            &tmpl.morphs[MORPH_ANKLE_DECR],
            (m.ankle_mm as f32 - 215.0) / 50.0,
            0.80,
        );

        let j_morphed: Vec<Vec3> = anchor_sets
            .iter()
            .enumerate()
            .map(|(ji, ids)| {
                let delta = avg_verts(&verts, ids).sub(base_anchor_centers[ji]);
                let c = tmpl.src_joints[ji].add(delta);
                // Keep sagittal joints cleanly on X=0
                if matches!(ji, J_PELVIS | J_CROTCH | J_WAIST | J_BUST | J_NECK_BASE | J_HEAD_TOP)
                {
                    Vec3::new(0.0, c.y, c.z)
                } else {
                    c
                }
            })
            .collect();

        // Pose-invariant anatomical deltoid/acromion half-width measured before Stage B arm posing
        let sh_j_pre = j_morphed[J_SHOULDER_L];
        let morphed_sh_half_x = verts
            .iter()
            .filter(|v| {
                v.y >= sh_j_pre.y - 0.015
                    && v.y <= sh_j_pre.y + 0.045
                    && v.x > 0.08
                    && v.x <= sh_j_pre.x + 0.052
            })
            .map(|v| v.x)
            .fold(sh_j_pre.x + 0.040, f32::max);

        // === STAGE B: Hierarchical 11-Bone Linear Blend Skinning (LBS) ===
        let sh_l = j_morphed[J_SHOULDER_L];
        let el_l = j_morphed[J_ELBOW_L];
        let wr_l = j_morphed[J_WRIST_L];
        let hip_l = j_morphed[J_HIP_L];
        let an_l = j_morphed[J_ANKLE_L];

        let (arm_abduct_deg, clav_lift_deg, upper_z, fore_z) = match pose {
            AvatarPose::APose => (28.0_f32, 0.0_f32, -0.015_f32, 0.105_f32),
            AvatarPose::ArmsRaised => (66.0_f32, 10.0_f32, 0.0_f32, 0.055_f32),
            AvatarPose::TPose => (84.0_f32, 13.0_f32, 0.0_f32, 0.025_f32),
        };
        let target_ang = arm_abduct_deg.to_radians();
        let target_upper_dir_l = Vec3::new(target_ang.sin(), -target_ang.cos(), upper_z).normalized();
        let target_fore_dir_l = Vec3::new(target_ang.sin(), -target_ang.cos(), fore_z).normalized();

        let upper_dir_l = el_l.sub(sh_l).normalized();
        let fore_dir_l = wr_l.sub(el_l).normalized();

        // Clavicle gentle lift for overhead / T poses around medial sternoclavicular pivot
        let clav_piv_l = Vec3::new(0.038, sh_l.y - 0.01, 0.01);
        let rot_clav_l = rot_axis_angle(Vec3::new(0.0, 0.0, 1.0), clav_lift_deg.to_radians());
        let xform_clav_l = Mat3x4::about_pivot(rot_clav_l, clav_piv_l);

        // Pose upper arm naturally and preserve gentle forward studio elbow flexion (never inverted)
        let sh_l_after_clav = xform_clav_l.apply(sh_l);
        let rot_upper_l = rot_from_to(xform_clav_l.apply_dir(upper_dir_l), target_upper_dir_l);
        let xform_upper_l =
            Mat3x4::compose(Mat3x4::about_pivot(rot_upper_l, sh_l_after_clav), xform_clav_l);

        let fore_after_upper = xform_upper_l.apply_dir(fore_dir_l);
        let el_after_upper = xform_upper_l.apply(el_l);
        let rot_fore_l = rot_from_to(fore_after_upper, target_fore_dir_l);
        let xform_fore_l =
            Mat3x4::compose(Mat3x4::about_pivot(rot_fore_l, el_after_upper), xform_upper_l);

        // Sleek studio leg stance (keep thighs clearly separated while centering feet under hip stance)
        let target_ankle_l = Vec3::new((hip_l.x * 1.15).max(0.108), an_l.y, hip_l.z);
        let cur_leg_dir_l = an_l.sub(hip_l).normalized();
        let tgt_leg_dir_l = target_ankle_l.sub(hip_l).normalized();
        let rot_leg_l = rot_from_to(cur_leg_dir_l, tgt_leg_dir_l);
        let xform_leg_l = Mat3x4::about_pivot(rot_leg_l, hip_l);

        // Right side mirrors cleanly across X = 0
        let xform_clav_r = xform_clav_l.mirror_x();
        let xform_upper_r = xform_upper_l.mirror_x();
        let xform_fore_r = xform_fore_l.mirror_x();
        let xform_leg_r = xform_leg_l.mirror_x();

        let bones = [
            Mat3x4::IDENTITY, // 0 Torso
            xform_clav_l,     // 1 ClavicleL
            xform_upper_l,    // 2 UpperArmL
            xform_fore_l,     // 3 ForearmL
            xform_clav_r,     // 4 ClavicleR
            xform_upper_r,    // 5 UpperArmR
            xform_fore_r,     // 6 ForearmR
            xform_leg_l,      // 7 ThighL
            xform_leg_l,      // 8 CalfL
            xform_leg_r,      // 9 ThighR
            xform_leg_r,      // 10 CalfR
        ];

        for i in 0..verts.len() {
            let p = verts[i];
            let b0 = tmpl.bone0[i] as usize;
            let b1 = tmpl.bone1[i] as usize;
            let w0 = tmpl.weight0[i];
            let p0 = bones[b0].apply(p);
            if w0 >= 0.999 || b0 == b1 {
                verts[i] = p0;
            } else {
                let p1 = bones[b1].apply(p);
                verts[i] = p0.scale(w0).add(p1.scale(1.0 - w0));
            }
        }

        // Update joint landmarks after posing
        let mut p_sh_l = xform_clav_l.apply(sh_l);
        let mut p_el_l = xform_upper_l.apply(el_l);
        let mut p_wr_l = xform_fore_l.apply(wr_l);
        let p_an_l = xform_leg_l.apply(an_l);
        let min_y_posed = verts.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
        for v in &mut verts {
            v.y -= min_y_posed;
        }
        p_sh_l.y -= min_y_posed;
        p_el_l.y -= min_y_posed;
        p_wr_l.y -= min_y_posed;
        let _ = p_el_l;

        // === STAGE C: Monotonic Piecewise Vertical Landmark + Shoulder + Sleeve Length Scaling ===
        // Measure current posed height anchors cleanly from actual morphed/posed mesh
        let s_head_top = verts.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);
        let scale_to_src = (s_head_top / tmpl.src_head_top_y.max(1.2)).clamp(0.85, 1.15);
        let src_anchors = [
            0.0_f32,
            (p_an_l.y - min_y_posed).max(0.045),
            tmpl.src_knee_y * scale_to_src,
            tmpl.src_crotch_y * scale_to_src,
            tmpl.src_hip_y * scale_to_src,
            tmpl.src_waist_y * scale_to_src,
            tmpl.src_bust_y * scale_to_src,
            (tmpl.src_neck_y * scale_to_src).max(tmpl.src_bust_y * scale_to_src + 0.12),
            ((tmpl.src_neck_y + 0.065) * scale_to_src).min(s_head_top - 0.10),
            s_head_top,
        ];
        let dst_anchors = [
            0.0_f32,
            ankle_y,
            knee_y,
            crotch_y,
            hip_y,
            waist_y,
            bust_y,
            shoulder_y,
            neck_y,
            head_top_y,
        ];

        // Lateral shoulder calibration using pose-invariant pre-LBS acromion/deltoid half-width
        let target_sh_x_ratio = (shoulder_half_x / morphed_sh_half_x.max(0.12)).clamp(0.78, 1.32);

        // Anchor shoulder pivot with the exact same flat-top Torso envelope so armpit/deltoid seam stays shear-free
        let sh_x_env = |y_val: f32, x_abs: f32| -> f32 {
            if y_val >= bust_y - 0.02 && y_val <= neck_y + 0.01 {
                let rise = smoothstep01((y_val - (bust_y - 0.02)) / 0.08);
                let fall =
                    smoothstep01((neck_y + 0.01 - y_val) / (neck_y + 0.01 - acromion_y).max(0.05));
                let radial = smoothstep01((x_abs - 0.058) / 0.06);
                rise * fall * radial
            } else {
                0.0
            }
        };
        let dst_sh_y = piecewise_map(p_sh_l.y, &src_anchors, &dst_anchors);
        let dst_sh_x =
            p_sh_l.x * (1.0 + (target_sh_x_ratio - 1.0) * sh_x_env(dst_sh_y, p_sh_l.x.abs()));
        let dst_sh_l = Vec3::new(dst_sh_x, dst_sh_y, p_sh_l.z);

        let arm_chord_l = p_wr_l.sub(p_sh_l);
        let src_arm_len = arm_chord_l.len().max(0.35);
        let arm_dir_l = arm_chord_l.scale(1.0 / src_arm_len);
        let dst_arm_len = (sleeve_len * 0.96).max(0.42);
        let h_wrist = 0.04_f32;
        let wrist_blend_start = (src_arm_len - h_wrist).max(0.10);
        let r_eff = (dst_arm_len - 0.5 * h_wrist) / (src_arm_len - 0.5 * h_wrist).max(0.10);

        for i in 0..verts.len() {
            let p = verts[i];
            let y_new = piecewise_map(p.y, &src_anchors, &dst_anchors);
            let x_new = p.x * (1.0 + (target_sh_x_ratio - 1.0) * sh_x_env(y_new, p.x.abs()));
            let p_torso = Vec3::new(x_new, y_new, p.z);

            let b0 = tmpl.bone0[i];
            let b1 = tmpl.bone1[i];
            let w0 = tmpl.weight0[i];
            let arm_l_w = bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_L, BONE_FOREARM_L);
            let arm_r_w = bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_R, BONE_FOREARM_R);
            let (arm_w, s) = if arm_l_w >= arm_r_w {
                (arm_l_w, 1.0_f32)
            } else {
                (arm_r_w, -1.0_f32)
            };

            if arm_w > 0.001 {
                let sh_src = Vec3::new(s * p_sh_l.x, p_sh_l.y, p_sh_l.z);
                let sh_dst = Vec3::new(s * dst_sh_l.x, dst_sh_l.y, dst_sh_l.z);
                let adir = Vec3::new(s * arm_dir_l.x, arm_dir_l.y, arm_dir_l.z).normalized();
                let rel = p.sub(sh_src);
                let d_along = rel.dot(adir);
                let perp = rel.sub(adir.scale(d_along));
                let scaled_along = if d_along <= 0.0 {
                    d_along
                } else if d_along <= wrist_blend_start {
                    d_along * r_eff
                } else if d_along <= src_arm_len {
                    let u = ((d_along - wrist_blend_start) / h_wrist).clamp(0.0, 1.0);
                    let u2 = u * u;
                    wrist_blend_start * r_eff
                        + h_wrist * (r_eff * u + (1.0 - r_eff) * (u2 * u - 0.5 * u2 * u2))
                } else {
                    // Translate complete sculpted hand/fingers rigidity past wrist with zero cutoff
                    dst_arm_len + (d_along - src_arm_len)
                };
                let p_arm = sh_dst.add(adir.scale(scaled_along)).add(perp);
                let sb = smoothstep01(arm_w);
                verts[i] = p_torso.scale(1.0 - sb).add(p_arm.scale(sb));
            } else {
                verts[i] = p_torso;
            }
        }

        // Convert closed quad mesh into outward-wound triangle index buffer once
        let mut indices = Vec::with_capacity(tmpl.quads.len() * 6);
        for &[a, b, c, d] in &tmpl.quads {
            indices.extend_from_slice(&[
                a as u32, b as u32, c as u32, a as u32, c as u32, d as u32,
            ]);
        }

        // Define posed limb axes in destination coordinates for slicing & radial calibration
        let arm_seg_l = (dst_sh_l, dst_sh_l.add(arm_dir_l.scale(dst_arm_len)));
        let arm_seg_r = (
            Vec3::new(-arm_seg_l.0.x, arm_seg_l.0.y, arm_seg_l.0.z),
            Vec3::new(-arm_seg_l.1.x, arm_seg_l.1.y, arm_seg_l.1.z),
        );
        let leg_top_x = verts
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                tmpl.region[*i] == REG_LEG_L && (v.y - (crotch_y - 0.03)).abs() < 0.03
            })
            .map(|(_, v)| v.x)
            .sum::<f32>()
            / (verts
                .iter()
                .enumerate()
                .filter(|(i, v)| {
                    tmpl.region[*i] == REG_LEG_L && (v.y - (crotch_y - 0.03)).abs() < 0.03
                })
                .count()
                .max(1) as f32);
        let leg_bot_x = verts
            .iter()
            .enumerate()
            .filter(|(i, v)| tmpl.region[*i] == REG_LEG_L && (v.y - ankle_y).abs() < 0.03)
            .map(|(_, v)| v.x)
            .sum::<f32>()
            / (verts
                .iter()
                .enumerate()
                .filter(|(i, v)| tmpl.region[*i] == REG_LEG_L && (v.y - ankle_y).abs() < 0.03)
                .count()
                .max(1) as f32);
        let leg_seg_l = (
            Vec3::new(leg_top_x.max(hip_rx * 0.42), hip_y - 0.01, 0.0),
            Vec3::new(leg_bot_x.max(hip_rx * 0.36), ankle_y, 0.0),
        );
        let leg_seg_r = (
            Vec3::new(-leg_seg_l.0.x, leg_seg_l.0.y, leg_seg_l.0.z),
            Vec3::new(-leg_seg_l.1.x, leg_seg_l.1.y, leg_seg_l.1.z),
        );

        // === STAGE D: Closed-Loop Mesh-Slice Circumference Calibration (2 iterations) ===
        let arm_axis_l = arm_seg_l.1.sub(arm_seg_l.0).normalized();
        let leg_axis_l = leg_seg_l.1.sub(leg_seg_l.0).normalized();

        for _ in 0..2 {
            // 1. Torso loops (bust, waist, hip)
            let (cur_bust, zc_b) =
                measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, bust_y);
            let (cur_waist, zc_w) =
                measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, waist_y);
            let (cur_hip, zc_h) =
                measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, hip_y);

            let sb = if cur_bust > 200.0 {
                ((m.bust_mm as f32) / cur_bust).clamp(0.75, 1.32)
            } else {
                1.0
            };
            let sw = if cur_waist > 200.0 {
                ((m.waist_mm as f32) / cur_waist).clamp(0.75, 1.32)
            } else {
                1.0
            };
            let sh = if cur_hip > 200.0 {
                ((m.hip_mm as f32) / cur_hip).clamp(0.75, 1.32)
            } else {
                1.0
            };

            let sigma_b = (shoulder_y - waist_y).max(0.14) * 0.36;
            let sigma_w = ((bust_y - hip_y).max(0.12) * 0.36).min(0.085);
            let sigma_h = (waist_y - crotch_y).max(0.12) * 0.40;

            for i in 0..verts.len() {
                let b0 = tmpl.bone0[i];
                let b1 = tmpl.bone1[i];
                let w0 = tmpl.weight0[i];
                let w_arm = (bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_L, BONE_FOREARM_L)
                    + bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_R, BONE_FOREARM_R))
                .clamp(0.0, 1.0);
                let torso_mask_w = smoothstep01(1.0 - w_arm);
                if torso_mask_w <= 1e-4 {
                    continue;
                }
                let p = verts[i];
                if p.y >= neck_y - 0.005 || p.y <= crotch_y - 0.16 {
                    continue;
                }
                let top_fade = smoothstep01((neck_y - 0.005 - p.y) / 0.065);
                let bot_fade = smoothstep01((p.y - (crotch_y - 0.16)) / 0.15);
                let wb = gaussian(p.y - bust_y, sigma_b) * top_fade;
                let ww = gaussian(p.y - waist_y, sigma_w);
                let wh = gaussian(p.y - hip_y, sigma_h) * bot_fade;
                let w_sum = (wb + ww + wh).max(1.0);
                let raw_delta = ((sb - 1.0) * wb + (sw - 1.0) * ww + (sh - 1.0) * wh) / w_sum;
                let s_rad = 1.0 + raw_delta * torso_mask_w * top_fade * bot_fade;
                let zc = (zc_b * wb + zc_w * ww + zc_h * wh) / (wb + ww + wh).max(1e-4);
                let x_new = p.x * s_rad;
                // Only prevent thigh-to-thigh crossover well below the perineum arch
                let safe_x =
                    if p.y < crotch_y - 0.045 && tmpl.region[i] == REG_LEG_L && x_new < 0.004 {
                        0.004
                    } else if p.y < crotch_y - 0.045 && tmpl.region[i] == REG_LEG_R && x_new > -0.004
                    {
                        -0.004
                    } else {
                        x_new
                    };
                verts[i] = Vec3::new(safe_x, p.y, zc + (p.z - zc) * s_rad);
            }

            // 2. Arm radial calibration (bicep & wrist)
            let p_bi = arm_seg_l.0.add(arm_axis_l.scale(dst_arm_len * 0.28));
            let p_wr = arm_seg_l.0.add(arm_axis_l.scale(dst_arm_len * 0.86));
            let cur_bi =
                measure_plane_loop(&verts, &indices, &tmpl.region, REG_ARM_L, p_bi, arm_axis_l, 0.14);
            let cur_wr =
                measure_plane_loop(&verts, &indices, &tmpl.region, REG_ARM_L, p_wr, arm_axis_l, 0.11);
            let s_bi = if cur_bi > 80.0 {
                ((m.bicep_mm as f32) / cur_bi).clamp(0.72, 1.35)
            } else {
                1.0
            };
            let s_wr = if cur_wr > 60.0 {
                ((m.wrist_mm as f32) / cur_wr).clamp(0.72, 1.35)
            } else {
                1.0
            };

            for i in 0..verts.len() {
                let b0 = tmpl.bone0[i];
                let b1 = tmpl.bone1[i];
                let w0 = tmpl.weight0[i];
                let arm_l_w = bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_L, BONE_FOREARM_L);
                let arm_r_w = bone_pair_weight(b0, b1, w0, BONE_UPPER_ARM_R, BONE_FOREARM_R);
                let (arm_w, seg) = if arm_l_w >= arm_r_w {
                    (arm_l_w, arm_seg_l)
                } else {
                    (arm_r_w, arm_seg_r)
                };
                let mask_w = smoothstep01(arm_w);
                if mask_w <= 1e-4 {
                    continue;
                }
                let ax = seg.1.sub(seg.0).normalized();
                let rel = verts[i].sub(seg.0);
                let d_al = rel.dot(ax);
                let t = d_al / dst_arm_len;
                if t <= 0.06 || t >= 0.99 {
                    continue;
                }
                let radial = rel.sub(ax.scale(d_al));
                let w_prox = (1.0 - (t - 0.28).max(0.0) / 0.58).clamp(0.0, 1.0);
                let w_dist = 1.0 - w_prox;
                let env = smoothstep01((t - 0.06) / 0.16)
                    * smoothstep01((0.99 - t) / 0.13)
                    * mask_w;
                let s_arm = 1.0 + ((s_bi - 1.0) * w_prox + (s_wr - 1.0) * w_dist) * env;
                verts[i] = seg.0.add(ax.scale(d_al)).add(radial.scale(s_arm));
            }

            // 3. Leg radial calibration (thigh, knee, ankle)
            let leg_len_l = leg_seg_l.1.sub(leg_seg_l.0).len().max(0.35);
            let p_th = leg_seg_l.0.add(leg_axis_l.scale(leg_len_l * 0.24));
            let p_kn = leg_seg_l.0.add(leg_axis_l.scale(leg_len_l * 0.52));
            let p_an = leg_seg_l.0.add(leg_axis_l.scale(leg_len_l * 0.92));
            let cur_th =
                measure_plane_loop(&verts, &indices, &tmpl.region, REG_LEG_L, p_th, leg_axis_l, 0.18);
            let cur_kn =
                measure_plane_loop(&verts, &indices, &tmpl.region, REG_LEG_L, p_kn, leg_axis_l, 0.14);
            let cur_an =
                measure_plane_loop(&verts, &indices, &tmpl.region, REG_LEG_L, p_an, leg_axis_l, 0.11);

            let s_th = if cur_th > 150.0 {
                ((m.thigh_mm as f32) / cur_th).clamp(0.75, 1.32)
            } else {
                1.0
            };
            let s_kn = if cur_kn > 100.0 {
                ((m.knee_mm as f32) / cur_kn).clamp(0.75, 1.32)
            } else {
                1.0
            };
            let s_an = if cur_an > 80.0 {
                ((m.ankle_mm as f32) / cur_an).clamp(0.75, 1.32)
            } else {
                1.0
            };

            for i in 0..verts.len() {
                let b0 = tmpl.bone0[i];
                let b1 = tmpl.bone1[i];
                let w0 = tmpl.weight0[i];
                let leg_l_w = bone_pair_weight(b0, b1, w0, BONE_THIGH_L, BONE_CALF_L);
                let leg_r_w = bone_pair_weight(b0, b1, w0, BONE_THIGH_R, BONE_CALF_R);
                let (leg_w, seg, s_sign) = if leg_l_w >= leg_r_w {
                    (leg_l_w, leg_seg_l, 1.0_f32)
                } else {
                    (leg_r_w, leg_seg_r, -1.0_f32)
                };
                let mask_w = smoothstep01(leg_w);
                if mask_w <= 1e-4 {
                    continue;
                }
                let p = verts[i];
                let prox_y_env = smoothstep01((crotch_y - 0.015 - p.y) / 0.11);
                let dist_y_env = smoothstep01((p.y - (ankle_y - 0.015)) / 0.07);
                let env = prox_y_env * dist_y_env * mask_w;
                if env <= 1e-4 {
                    continue;
                }
                let leg_len = seg.1.sub(seg.0).len().max(0.35);
                let ax = seg.1.sub(seg.0).normalized();
                let rel = p.sub(seg.0);
                let d_al = rel.dot(ax);
                let t = (d_al / leg_len).clamp(0.0, 1.0);
                let wt = ((0.52 - t) / 0.28).clamp(0.0, 1.0);
                let wa = ((t - 0.52) / 0.40).clamp(0.0, 1.0);
                let wk = (1.0 - wt - wa).clamp(0.0, 1.0);
                let s_leg = 1.0 + ((s_th - 1.0) * wt + (s_kn - 1.0) * wk + (s_an - 1.0) * wa) * env;
                let radial = rel.sub(ax.scale(d_al));
                let mut np = seg.0.add(ax.scale(d_al)).add(radial.scale(s_leg));
                if p.y < crotch_y - 0.045 && np.x * s_sign < 0.004 {
                    np.x = s_sign * 0.004;
                }
                verts[i] = np;
            }
        }

        // Seat feet cleanly onto studio floor y = 0.0 m
        let final_min_y = verts.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
        for v in &mut verts {
            v.y -= final_min_y;
        }
        let shift_y = final_min_y;
        let arm_seg_l = (
            Vec3::new(arm_seg_l.0.x, arm_seg_l.0.y - shift_y, arm_seg_l.0.z),
            Vec3::new(arm_seg_l.1.x, arm_seg_l.1.y - shift_y, arm_seg_l.1.z),
        );
        let arm_seg_r = (
            Vec3::new(arm_seg_r.0.x, arm_seg_r.0.y - shift_y, arm_seg_r.0.z),
            Vec3::new(arm_seg_r.1.x, arm_seg_r.1.y - shift_y, arm_seg_r.1.z),
        );
        let leg_seg_l = (
            Vec3::new(leg_seg_l.0.x, leg_seg_l.0.y - shift_y, leg_seg_l.0.z),
            Vec3::new(leg_seg_l.1.x, leg_seg_l.1.y - shift_y, leg_seg_l.1.z),
        );
        let leg_seg_r = (
            Vec3::new(leg_seg_r.0.x, leg_seg_r.0.y - shift_y, leg_seg_r.0.z),
            Vec3::new(leg_seg_r.1.x, leg_seg_r.1.y - shift_y, leg_seg_r.1.z),
        );

        let bust_y_f = bust_y - shift_y;
        let waist_y_f = waist_y - shift_y;
        let hip_y_f = hip_y - shift_y;
        let crotch_y_f = crotch_y - shift_y;
        let shoulder_y_f = shoulder_y - shift_y;
        let acromion_y_f = acromion_y - shift_y;
        let neck_y_f = neck_y - shift_y;
        let ankle_y_f = ankle_y - shift_y;
        let knee_y_f = knee_y - shift_y;
        let final_max_y = verts.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);

        // Final measured garment-fit telemetry from exact posed & calibrated mesh
        let (meas_bust, _) =
            measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, bust_y_f);
        let (meas_waist, _) =
            measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, waist_y_f);
        let (meas_hip, _) =
            measure_horizontal_loop(&verts, &indices, &tmpl.region, REG_TORSO, hip_y_f);
        let p_bi = arm_seg_l.0.add(arm_axis_l.scale(dst_arm_len * 0.28));
        let leg_len_l = leg_seg_l.1.sub(leg_seg_l.0).len().max(0.35);
        let p_th = leg_seg_l.0.add(leg_axis_l.scale(leg_len_l * 0.24));
        let meas_bicep =
            measure_plane_loop(&verts, &indices, &tmpl.region, REG_ARM_L, p_bi, arm_axis_l, 0.14);
        let meas_thigh =
            measure_plane_loop(&verts, &indices, &tmpl.region, REG_LEG_L, p_th, leg_axis_l, 0.18);
        let meas_sh_x = verts
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                (tmpl.region[*i] == REG_TORSO || tmpl.region[*i] == REG_ARM_L)
                    && (v.y - acromion_y_f).abs() < 0.035
            })
            .map(|(_, v)| v.x.abs())
            .fold(0.18_f32, f32::max);

        let measured = AvatarMeasuredMetrics {
            height_mm: final_max_y * 1000.0,
            bust_mm: meas_bust,
            waist_mm: meas_waist,
            hip_mm: meas_hip,
            shoulder_mm: meas_sh_x * 2000.0,
            bicep_mm: meas_bicep,
            thigh_mm: meas_thigh,
        };

        // Pack flat GPU buffers, recompute smooth normals, and build exact BVH SDF collider
        let mut positions = Vec::with_capacity(verts.len() * 3);
        for v in &verts {
            positions.extend_from_slice(&[v.x, v.y, v.z]);
        }
        let mut normals = vec![0.0_f32; positions.len()];
        recompute_smooth_normals(&positions, &indices, &mut normals);

        let bvh = MeshSdfBvh::build(&positions, &indices);
        let torso_mask: Vec<f32> = tmpl
            .region
            .iter()
            .map(|&r| if r == REG_TORSO { 1.0 } else { 0.0 })
            .collect();
        let render_mesh = AvatarMesh {
            positions,
            normals,
            torso_mask,
            indices: indices.clone(),
        };

        // Extract compact surface rings & limb radius lookup tables directly from calibrated mesh
        let (torso_surface_rings, torso_ring_profile) = extract_torso_tables(
            &verts,
            &indices,
            &tmpl.region,
            crotch_y_f,
            neck_y_f,
            (waist_rx, waist_rz),
        );
        let arm_radii = extract_limb_radii(&verts, arm_seg_l, REG_ARM_L, &tmpl.region, bicep_r, wrist_r);
        let leg_radii = extract_limb_radii(&verts, leg_seg_l, REG_LEG_L, &tmpl.region, thigh_r, ankle_r);

        Self {
            pose,
            body_form,
            ankle_y: ankle_y_f,
            knee_y: knee_y_f,
            crotch_y: crotch_y_f,
            hip_y: hip_y_f,
            waist_y: waist_y_f,
            bust_y: bust_y_f,
            acromion_y: acromion_y_f,
            shoulder_y: shoulder_y_f,
            neck_y: neck_y_f,
            head_y: head_y - shift_y,
            waist_rx,
            waist_rz,
            bust_rx,
            bust_rz,
            hip_rx,
            hip_rz,
            shoulder_half_x,
            bicep_r,
            wrist_r,
            sleeve_len,
            thigh_r,
            knee_r,
            ankle_r,
            measured,
            render_mesh,
            bvh,
            torso_surface_rings,
            torso_ring_profile,
            arm_seg_l,
            arm_seg_r,
            arm_radii,
            leg_seg_l,
            leg_seg_r,
            leg_radii,
        }
    }

    pub fn torso_profile_at_y(&self, y: f32) -> (f32, f32, f32) {
        let span = (self.neck_y - self.crotch_y).max(0.05);
        let f = ((y - self.crotch_y) / span).clamp(0.0, 1.0) * ((TORSO_RINGS - 1) as f32);
        let r0 = (f.floor() as usize).min(TORSO_RINGS - 1);
        let r1 = (r0 + 1).min(TORSO_RINGS - 1);
        let t = f - (r0 as f32);
        let a = self.torso_ring_profile[r0];
        let b = self.torso_ring_profile[r1];
        (lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
    }

    pub fn torso_radii_at_y(&self, y: f32) -> (f32, f32) {
        let (rx, rz, _) = self.torso_profile_at_y(y);
        (rx, rz)
    }

    /// Bilinear lookup onto the calibrated real mannequin torso surface (`+X = cos(ang)`, `+Z = sin(ang)`).
    pub fn torso_surface_xz(&self, y: f32, ang: f32) -> (f32, f32) {
        let span = (self.neck_y - self.crotch_y).max(0.05);
        let fy = ((y - self.crotch_y) / span).clamp(0.0, 1.0) * ((TORSO_RINGS - 1) as f32);
        let r0 = (fy.floor() as usize).min(TORSO_RINGS - 1);
        let r1 = (r0 + 1).min(TORSO_RINGS - 1);
        let ty = fy - (r0 as f32);

        let a_norm = ang.rem_euclid(std::f32::consts::TAU);
        let fa = (a_norm / std::f32::consts::TAU) * (TORSO_SEGS as f32);
        let s0 = (fa.floor() as usize) % TORSO_SEGS;
        let s1 = (s0 + 1) % TORSO_SEGS;
        let ta = fa - fa.floor();

        let p00 = self.torso_surface_rings[r0][s0];
        let p01 = self.torso_surface_rings[r0][s1];
        let p10 = self.torso_surface_rings[r1][s0];
        let p11 = self.torso_surface_rings[r1][s1];

        let p0 = p00.add(p01.sub(p00).scale(ta));
        let p1 = p10.add(p11.sub(p10).scale(ta));
        let p = p0.add(p1.sub(p0).scale(ty));
        (p.x, p.z)
    }

    #[inline]
    pub fn arm_radius_at(&self, t: f32) -> f32 {
        sample_radii(&self.arm_radii, t)
    }

    #[inline]
    pub fn leg_radius_at(&self, t: f32) -> f32 {
        sample_radii(&self.leg_radii, t)
    }

    pub fn arm_segment(&self, side: f32) -> (Vec3, Vec3) {
        if side >= 0.0 {
            self.arm_seg_l
        } else {
            self.arm_seg_r
        }
    }

    pub fn leg_segment(&self, side: f32) -> (Vec3, Vec3) {
        if side >= 0.0 {
            self.leg_seg_l
        } else {
            self.leg_seg_r
        }
    }

    /// Exact triangle-mesh BVH SDF query (`distance_m`, `outward_unit_normal`).
    #[inline]
    pub fn eval_sdf(&self, p: Vec3) -> (f32, Vec3) {
        self.bvh.signed_distance(p)
    }

    /// Bounded exact BVH SDF query within `max_dist` meters (returns `None` when farther).
    #[inline]
    pub fn eval_sdf_within(&self, p: Vec3, max_dist: f32) -> Option<(f32, Vec3)> {
        self.bvh.signed_distance_within(p, max_dist)
    }

    /// Returns the cohesive watertight studio mannequin mesh (shared identically with BVH collider).
    pub fn build_render_mesh(&self) -> AvatarMesh {
        self.render_mesh.clone()
    }
}

#[derive(Clone, Copy)]
struct Mat3x4 {
    r: [Vec3; 3],
    t: Vec3,
}

impl Mat3x4 {
    const IDENTITY: Self = Self {
        r: [
            Vec3 { x: 1.0, y: 0.0, z: 0.0 },
            Vec3 { x: 0.0, y: 1.0, z: 0.0 },
            Vec3 { x: 0.0, y: 0.0, z: 1.0 },
        ],
        t: Vec3::ZERO,
    };

    fn about_pivot(rot: [Vec3; 3], pivot: Vec3) -> Self {
        let r0p = rot[0].dot(pivot);
        let r1p = rot[1].dot(pivot);
        let r2p = rot[2].dot(pivot);
        Self {
            r: rot,
            t: Vec3::new(pivot.x - r0p, pivot.y - r1p, pivot.z - r2p),
        }
    }

    fn apply(self, p: Vec3) -> Vec3 {
        Vec3::new(
            self.r[0].dot(p) + self.t.x,
            self.r[1].dot(p) + self.t.y,
            self.r[2].dot(p) + self.t.z,
        )
    }

    fn apply_dir(self, d: Vec3) -> Vec3 {
        Vec3::new(self.r[0].dot(d), self.r[1].dot(d), self.r[2].dot(d))
    }

    fn compose(outer: Self, inner: Self) -> Self {
        let mut r = [Vec3::ZERO; 3];
        for i in 0..3 {
            let col0 = Vec3::new(inner.r[0].x, inner.r[1].x, inner.r[2].x);
            let col1 = Vec3::new(inner.r[0].y, inner.r[1].y, inner.r[2].y);
            let col2 = Vec3::new(inner.r[0].z, inner.r[1].z, inner.r[2].z);
            r[i] = Vec3::new(
                outer.r[i].dot(col0),
                outer.r[i].dot(col1),
                outer.r[i].dot(col2),
            );
        }
        let t = outer.apply(inner.t);
        Self { r, t }
    }

    fn mirror_x(self) -> Self {
        Self {
            r: [
                Vec3::new(self.r[0].x, -self.r[0].y, -self.r[0].z),
                Vec3::new(-self.r[1].x, self.r[1].y, self.r[1].z),
                Vec3::new(-self.r[2].x, self.r[2].y, self.r[2].z),
            ],
            t: Vec3::new(-self.t.x, self.t.y, self.t.z),
        }
    }
}

fn rot_axis_angle(axis_in: Vec3, angle: f32) -> [Vec3; 3] {
    if angle.abs() < 1e-6 {
        return Mat3x4::IDENTITY.r;
    }
    let a = axis_in.normalized();
    let c = angle.cos();
    let s = angle.sin();
    let oc = 1.0 - c;
    [
        Vec3::new(c + a.x * a.x * oc, a.x * a.y * oc - a.z * s, a.x * a.z * oc + a.y * s),
        Vec3::new(a.y * a.x * oc + a.z * s, c + a.y * a.y * oc, a.y * a.z * oc - a.x * s),
        Vec3::new(a.z * a.x * oc - a.y * s, a.z * a.y * oc + a.x * s, c + a.z * a.z * oc),
    ]
}

fn rot_from_to(from_in: Vec3, to_in: Vec3) -> [Vec3; 3] {
    let a = from_in.normalized();
    let b = to_in.normalized();
    let d = a.dot(b).clamp(-1.0, 1.0);
    if d > 0.9999 {
        return Mat3x4::IDENTITY.r;
    }
    if d < -0.9999 {
        let ortho = if a.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        return rot_axis_angle(a.cross(ortho), std::f32::consts::PI);
    }
    let axis = a.cross(b);
    let ang = d.acos();
    rot_axis_angle(axis, ang)
}

fn find_nearest_verts(verts: &[Vec3], target: Vec3, k: usize) -> Vec<usize> {
    let mut ranked: Vec<(f32, usize)> = verts
        .iter()
        .enumerate()
        .map(|(i, v)| (v.sub(target).len_sq(), i))
        .collect();
    let nth = k.min(ranked.len().saturating_sub(1));
    ranked.select_nth_unstable_by(nth, |a, b| {
        a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.into_iter().take(k).map(|(_, i)| i).collect()
}

fn avg_verts(verts: &[Vec3], ids: &[usize]) -> Vec3 {
    let mut acc = Vec3::ZERO;
    for &i in ids {
        acc = acc.add(verts[i]);
    }
    acc.scale(1.0 / (ids.len().max(1) as f32))
}

fn apply_morph(verts: &mut [Vec3], deltas: &[MorphDelta], w: f32) {
    if w.abs() < 1e-4 {
        return;
    }
    for d in deltas {
        let p = &mut verts[d.vid as usize];
        p.x += w * d.dx;
        p.y += w * d.dy;
        p.z += w * d.dz;
    }
}

fn apply_signed_morph_pair(
    verts: &mut [Vec3],
    incr: &[MorphDelta],
    decr: &[MorphDelta],
    val: f32,
    cap: f32,
) {
    if val > 0.01 {
        apply_morph(verts, incr, val.min(cap));
    } else if val < -0.01 {
        apply_morph(verts, decr, (-val).min(cap));
    }
}

fn piecewise_map(y: f32, src: &[f32], dst: &[f32]) -> f32 {
    if y <= src[0] {
        return dst[0] + (y - src[0]);
    }
    let last = src.len() - 1;
    if y >= src[last] {
        return dst[last] + (y - src[last]);
    }
    for k in 0..last {
        if y <= src[k + 1] {
            let span = (src[k + 1] - src[k]).max(1e-5);
            let t = (y - src[k]) / span;
            return dst[k] + (dst[k + 1] - dst[k]) * t;
        }
    }
    y
}

#[inline]
fn gaussian(dx: f32, sigma: f32) -> f32 {
    let s = sigma.max(0.02);
    (-(dx * dx) / (s * s)).exp()
}

fn measure_horizontal_loop(
    verts: &[Vec3],
    indices: &[u32],
    region: &[u8],
    req_region: u8,
    y_target: f32,
) -> (f32, f32) {
    let mut sum_len_m = 0.0_f32;
    let mut sum_z_wt = 0.0_f32;
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if region[i0] != req_region && region[i1] != req_region && region[i2] != req_region {
            continue;
        }
        let ps = [verts[i0], verts[i1], verts[i2]];
        if let Some((a, b)) = intersect_tri_plane(ps, Vec3::new(0.0, y_target, 0.0), Vec3::new(0.0, 1.0, 0.0)) {
            let seg = b.sub(a).len();
            sum_len_m += seg;
            sum_z_wt += seg * 0.5 * (a.z + b.z);
        }
    }
    let zc = if sum_len_m > 1e-4 {
        sum_z_wt / sum_len_m
    } else {
        0.0
    };
    (sum_len_m * 1000.0, zc)
}

fn measure_plane_loop(
    verts: &[Vec3],
    indices: &[u32],
    region: &[u8],
    req_region: u8,
    origin: Vec3,
    normal: Vec3,
    max_rad_m: f32,
) -> f32 {
    let max_r2 = max_rad_m * max_rad_m;
    let mut sum_len_m = 0.0_f32;
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if region[i0] != req_region && region[i1] != req_region && region[i2] != req_region {
            continue;
        }
        let ps = [verts[i0], verts[i1], verts[i2]];
        if let Some((a, b)) = intersect_tri_plane(ps, origin, normal) {
            let mid = a.add(b).scale(0.5);
            if mid.sub(origin).len_sq() <= max_r2 {
                sum_len_m += b.sub(a).len();
            }
        }
    }
    sum_len_m * 1000.0
}

#[inline]
fn intersect_tri_plane(ps: [Vec3; 3], origin: Vec3, normal: Vec3) -> Option<(Vec3, Vec3)> {
    let ds = [
        ps[0].sub(origin).dot(normal),
        ps[1].sub(origin).dot(normal),
        ps[2].sub(origin).dot(normal),
    ];
    let mut pts = [Vec3::ZERO; 2];
    let mut cnt = 0usize;
    for &(ia, ib) in &[(0, 1), (1, 2), (2, 0)] {
        let da = ds[ia];
        let db = ds[ib];
        if (da >= 0.0 && db < 0.0) || (da < 0.0 && db >= 0.0) {
            let t = da / (da - db);
            if cnt < 2 {
                pts[cnt] = ps[ia].add(ps[ib].sub(ps[ia]).scale(t));
                cnt += 1;
            }
        }
    }
    if cnt == 2 {
        Some((pts[0], pts[1]))
    } else {
        None
    }
}

fn extract_torso_tables(
    verts: &[Vec3],
    indices: &[u32],
    region: &[u8],
    crotch_y: f32,
    neck_y: f32,
    fallback_rx_rz: (f32, f32),
) -> (Vec<[Vec3; TORSO_SEGS]>, Vec<(f32, f32, f32)>) {
    let mut rings = Vec::with_capacity(TORSO_RINGS);
    let mut profiles = Vec::with_capacity(TORSO_RINGS);
    let mut segs: Vec<(Vec3, Vec3)> = Vec::with_capacity(128);

    for r in 0..TORSO_RINGS {
        let t = (r as f32) / ((TORSO_RINGS - 1) as f32);
        let y = lerp(crotch_y + 0.015, neck_y - 0.005, t);
        segs.clear();
        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut z_min = f32::INFINITY;
        let mut z_max = f32::NEG_INFINITY;
        let mut z_sum = 0.0_f32;
        let mut w_sum = 0.0_f32;

        for tri in indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if region[i0] != REG_TORSO && region[i1] != REG_TORSO && region[i2] != REG_TORSO {
                continue;
            }
            let ps = [verts[i0], verts[i1], verts[i2]];
            if let Some((a, b)) =
                intersect_tri_plane(ps, Vec3::new(0.0, y, 0.0), Vec3::new(0.0, 1.0, 0.0))
            {
                let l = b.sub(a).len();
                segs.push((a, b));
                for p in [a, b] {
                    x_min = x_min.min(p.x);
                    x_max = x_max.max(p.x);
                    z_min = z_min.min(p.z);
                    z_max = z_max.max(p.z);
                }
                z_sum += l * 0.5 * (a.z + b.z);
                w_sum += l;
            }
        }

        let zc = if w_sum > 1e-4 { z_sum / w_sum } else { 0.0 };
        let rx = if x_max > x_min {
            0.5 * (x_max - x_min)
        } else {
            fallback_rx_rz.0
        };
        let rz = if z_max > z_min {
            0.5 * (z_max - z_min)
        } else {
            fallback_rx_rz.1
        };
        profiles.push((rx.max(0.032), rz.max(0.032), zc));

        let mut ring = [Vec3::ZERO; TORSO_SEGS];
        for s in 0..TORSO_SEGS {
            let ang = (s as f32) * std::f32::consts::TAU / (TORSO_SEGS as f32);
            let dir_x = ang.cos();
            let dir_z = ang.sin();
            let mut best_t = 0.0_f32;
            for &(a, b) in &segs {
                let ax = a.x;
                let az = a.z - zc;
                let bx = b.x - a.x;
                let bz = (b.z - zc) - az;
                let denom = dir_x * bz - dir_z * bx;
                if denom.abs() < 1e-6 {
                    continue;
                }
                let u = (ax * dir_z - az * dir_x) / denom;
                let ray_t = (ax * bz - az * bx) / denom;
                if (-0.02..=1.02).contains(&u) && ray_t > best_t {
                    best_t = ray_t;
                }
            }
            if best_t > 0.02 {
                ring[s] = Vec3::new(best_t * dir_x, y, zc + best_t * dir_z);
            } else {
                ring[s] = Vec3::new(rx * dir_x, y, zc + rz * dir_z);
            }
        }
        rings.push(ring);
    }

    (rings, profiles)
}

fn extract_limb_radii(
    verts: &[Vec3],
    seg: (Vec3, Vec3),
    req_reg: u8,
    region: &[u8],
    r_start_fallback: f32,
    r_end_fallback: f32,
) -> Vec<f32> {
    let ab = seg.1.sub(seg.0);
    let len = ab.len().max(0.20);
    let ax = ab.scale(1.0 / len);
    let mut sum = vec![0.0_f32; LIMB_BINS];
    let mut cnt = vec![0u32; LIMB_BINS];

    for (i, &p) in verts.iter().enumerate() {
        if region[i] != req_reg {
            continue;
        }
        let rel = p.sub(seg.0);
        let t = rel.dot(ax) / len;
        if (0.0..=1.0).contains(&t) {
            let radial = rel.sub(ax.scale(rel.dot(ax))).len();
            if radial < 0.22 {
                let bin = ((t * (LIMB_BINS as f32 - 1.0)).round() as usize).min(LIMB_BINS - 1);
                sum[bin] += radial;
                cnt[bin] += 1;
            }
        }
    }

    (0..LIMB_BINS)
        .map(|k| {
            if cnt[k] > 0 {
                sum[k] / (cnt[k] as f32)
            } else {
                let u = (k as f32) / ((LIMB_BINS - 1) as f32);
                lerp(r_start_fallback, r_end_fallback, u)
            }
        })
        .collect()
}

#[inline]
fn sample_radii(bins: &[f32], t: f32) -> f32 {
    if bins.is_empty() {
        return 0.04;
    }
    let f = t.clamp(0.0, 1.0) * ((bins.len() - 1) as f32);
    let i0 = (f.floor() as usize).min(bins.len() - 1);
    let i1 = (i0 + 1).min(bins.len() - 1);
    lerp(bins[i0], bins[i1], f - (i0 as f32))
}

fn recompute_smooth_normals(positions: &[f32], indices: &[u32], normals: &mut [f32]) {
    for n in normals.iter_mut() {
        *n = 0.0;
    }
    for tri in indices.chunks_exact(3) {
        let i0 = tri[0] as usize * 3;
        let i1 = tri[1] as usize * 3;
        let i2 = tri[2] as usize * 3;
        let p0 = Vec3::new(positions[i0], positions[i0 + 1], positions[i0 + 2]);
        let p1 = Vec3::new(positions[i1], positions[i1 + 1], positions[i1 + 2]);
        let p2 = Vec3::new(positions[i2], positions[i2 + 1], positions[i2 + 2]);
        let fn_v = p1.sub(p0).cross(p2.sub(p0));
        for &idx in &[i0, i1, i2] {
            normals[idx] += fn_v.x;
            normals[idx + 1] += fn_v.y;
            normals[idx + 2] += fn_v.z;
        }
    }
    for chunk in normals.chunks_exact_mut(3) {
        let n = Vec3::new(chunk[0], chunk[1], chunk[2]).normalized();
        chunk[0] = n.x;
        chunk[1] = n.y;
        chunk[2] = n.z;
    }
}

fn ellipse_radii_from_circ(circ_m: f32, aspect_rx_over_rz: f32) -> (f32, f32) {
    let k = aspect_rx_over_rz;
    let h = ((k - 1.0) / (k + 1.0)).powi(2);
    let factor =
        std::f32::consts::PI * (k + 1.0) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()));
    let rz = (circ_m / factor).max(0.05);
    (rz * k, rz)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn smoothstep01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn bone_pair_weight(b0: u8, b1: u8, w0: f32, target_a: u8, target_b: u8) -> f32 {
    let wa = if b0 == target_a || b0 == target_b {
        w0
    } else {
        0.0
    };
    let wb = if b1 != b0 && (b1 == target_a || b1 == target_b) {
        1.0 - w0
    } else {
        0.0
    };
    wa + wb
}
