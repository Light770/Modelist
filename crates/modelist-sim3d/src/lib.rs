pub mod avatar;
pub(crate) mod arrange;
pub mod bvh;
pub mod closures;
pub(crate) mod collide;
pub mod cosserat;
pub mod diagnostics;
pub mod drawstring;
pub mod mannequin_data;
pub mod material;
#[cfg(test)]
mod material_tests;
pub mod mesher;
pub mod seam_physics;
pub mod self_collide;
pub mod sizing_map;
pub mod vbd;
pub mod xpbd;

#[cfg(test)]
mod tests {
    use crate::avatar::{AvatarBodyForm, AvatarPose, ParametricAvatar, Vec3};
    use crate::mannequin_data::{
        get_template, BONE_FOREARM_L, BONE_UPPER_ARM_L, REG_ARM_L, REG_ARM_R, REG_LEG_L,
        REG_LEG_R, REG_TORSO,
    };
    use crate::mesher::mesh_pattern_piece;
    use crate::xpbd::ClothSimSession;
    use modelist_core::blocks::{build_default_variables, build_template_pieces};
    use modelist_core::model::{det_id, BodyMeasurements, EaseTable, FabricSpec, ParametricOp};

    #[test]
    fn test_cdt_meshing_area_and_quality() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, _, _, _) = build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        for p in &pieces {
            let pm = mesh_pattern_piece(p, false, false, 22.0).expect("CDT mesh must succeed");
            assert!(pm.triangles.len() >= 10);
            let orig_area = p.seam_loop.signed_area().abs();
            let rel = (pm.total_area_mm2 - orig_area).abs() / orig_area.max(1.0);
            assert!(rel < 0.04, "{} mesh area error {rel}", p.name);
            assert!(pm.min_angle_deg > 2.0, "Degenerate sliver triangle detected");
        }
    }

    #[test]
    fn test_avatar_sdf_matches_render_mesh() {
        for form in [AvatarBodyForm::Female, AvatarBodyForm::Male] {
            for size in ["S", "M", "XL"] {
                let m = BodyMeasurements::from_size_preset(size);
                for pose in [AvatarPose::APose, AvatarPose::ArmsRaised, AvatarPose::TPose] {
                    let av = ParametricAvatar::from_measurements_with_form(&m, pose, form);
                    let mesh = av.build_render_mesh();
                    assert!(
                        mesh.positions.len() / 3 >= 10_000,
                        "Expected real high-res mannequin mesh"
                    );
                    let mut max_abs_sdf = 0.0_f32;
                    for (vi, c) in mesh.positions.chunks_exact(3).enumerate() {
                        // Sample all verts on base Size M, stride 5 across extra variants for fast CI
                        if !(size == "M" && form == AvatarBodyForm::Female) && vi % 5 != 0 {
                            continue;
                        }
                        let v = Vec3::new(c[0], c[1], c[2]);
                        let (sdf, _) = av.eval_sdf(v);
                        let abs_d = sdf.abs();
                        if abs_d > max_abs_sdf {
                            max_abs_sdf = abs_d;
                        }
                    }
                    assert!(
                        max_abs_sdf <= 0.001,
                        "{form:?} Size {size} pose {pose:?}: vertex diverged {max_abs_sdf:.5} m from BVH collision SDF!"
                    );
                }
            }
        }
    }

    #[test]
    fn test_avatar_anthropometric_accuracy() {
        for (size, form) in [
            ("S", AvatarBodyForm::Female),
            ("M", AvatarBodyForm::Female),
            ("XL", AvatarBodyForm::Female),
            ("M", AvatarBodyForm::Neutral),
            ("M", AvatarBodyForm::Male),
        ] {
            let m = BodyMeasurements::from_size_preset(size);
            let av = ParametricAvatar::from_measurements_with_form(&m, AvatarPose::APose, form);
            let meas = av.measured;
            println!(
                "[avatar {form:?} {size}] height={:.0}mm bust={:.1}/{:.0} waist={:.1}/{:.0} hip={:.1}/{:.0} sh={:.1}/{:.0} arm={:.1}/{:.0} thigh={:.1}/{:.0}",
                meas.height_mm,
                meas.bust_mm, m.bust_mm,
                meas.waist_mm, m.waist_mm,
                meas.hip_mm, m.hip_mm,
                meas.shoulder_mm, m.shoulder_width_mm,
                meas.bicep_mm, m.bicep_mm,
                meas.thigh_mm, m.thigh_mm,
            );
            assert!(
                (meas.bust_mm - m.bust_mm as f32).abs() <= 4.0,
                "{form:?} {size} bust mismatch: {:.1} vs {:.1}",
                meas.bust_mm,
                m.bust_mm
            );
            assert!(
                (meas.waist_mm - m.waist_mm as f32).abs() <= 4.0,
                "{form:?} {size} waist mismatch: {:.1} vs {:.1}",
                meas.waist_mm,
                m.waist_mm
            );
            assert!(
                (meas.hip_mm - m.hip_mm as f32).abs() <= 4.0,
                "{form:?} {size} hip mismatch: {:.1} vs {:.1}",
                meas.hip_mm,
                m.hip_mm
            );
        }
    }

    #[test]
    fn test_xpbd_drape_stability_all_three_blocks() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let fabric = FabricSpec::from_preset("cotton_poplin");
        let mut failures = Vec::new();
        for tmpl in ["bodice_sleeve", "aline_skirt", "trouser_block"] {
            let vars = build_default_variables(tmpl);
            let (pieces, stitching, _, _) =
                build_template_pieces(tmpl, &m, &e, &vars, &[], &[]);
            let mut sim =
                ClothSimSession::new(&pieces, &stitching, &m, &fabric, AvatarPose::APose);
            sim.step(40);
            let stats = sim.stats();
            let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
            let avg_strain: f32 =
                sim.flat_strain_pct.iter().sum::<f32>() / sim.flat_strain_pct.len().max(1) as f32;
            let min_y = sim
                .flat_positions
                .chunks_exact(3)
                .map(|c| c[1])
                .fold(f32::INFINITY, f32::min);
            let max_y = sim
                .flat_positions
                .chunks_exact(3)
                .map(|c| c[1])
                .fold(f32::NEG_INFINITY, f32::max);
            println!(
                "[{tmpl}] verts={} tris={} sew={} avg_gap={:.2}mm max_gap={:.2}mm avg_strain={:.2}% max_strain={:.1}% avg_clear={:.1}mm y=[{:.2}..{:.2}]m (waist={:.2}, shoulder={:.2})",
                stats.vertex_count,
                stats.triangle_count,
                stats.sew_pair_count,
                avg_gap_mm,
                max_gap_mm,
                avg_strain,
                stats.max_strain_pct,
                stats.avg_clearance_mm,
                min_y,
                max_y,
                sim.avatar.waist_y,
                sim.avatar.shoulder_y
            );
            assert!(stats.vertex_count > 100, "{tmpl} too few verts");
            assert!(stats.triangle_count > 150, "{tmpl} too few tris");
            assert!(stats.sew_pair_count > 10, "{tmpl} missing sew pairs");
            assert!(sim.flat_positions.iter().all(|v| v.is_finite()));
            assert!(sim.flat_normals.iter().all(|v| v.is_finite()));
            assert!(
                stats.min_clearance_mm >= 0.0,
                "{tmpl} cloth penetrated avatar: {}",
                stats.min_clearance_mm
            );
            if avg_gap_mm >= 2.5 || max_gap_mm >= 8.0 {
                failures.push(format!(
                    "{tmpl} seam gap too large: avg={avg_gap_mm:.2}mm, max={max_gap_mm:.2}mm"
                ));
            }
            if max_y <= sim.avatar.waist_y - 0.08 {
                failures.push(format!("{tmpl} fell off waist: max_y={max_y}"));
            }
            if tmpl == "bodice_sleeve" {
                if max_y < sim.avatar.shoulder_y - 0.04 || min_y < sim.avatar.crotch_y - 0.05 {
                    failures.push(format!(
                        "bodice_sleeve vertical bounds off: y=[{min_y:.2}..{max_y:.2}] waist={:.2} shoulder={:.2}",
                        sim.avatar.waist_y, sim.avatar.shoulder_y
                    ));
                }
            }
            // Woven blocks: low mean strain; the 99th percentile allows seam corners and dart tips.
            if avg_strain > 4.0 || stats.max_strain_pct > 40.0 {
                failures.push(format!(
                    "{tmpl} strain too high: avg={avg_strain:.2}%, p99={:.1}%",
                    stats.max_strain_pct
                ));
            }
            let q = sim.drape_quality();
            if q.poke_through_count > 5 || q.sharp_crease_pct > 5.0 {
                failures.push(format!("{tmpl} drape quality: {q:?}"));
            }
        }
        assert!(failures.is_empty(), "Drape failures: {failures:?}");
    }

    fn drape(tmpl: &str, preset: &str, frames: u32) -> ClothSimSession {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables(tmpl);
        let (pieces, stitching, _, _) = build_template_pieces(tmpl, &m, &e, &vars, &[], &[]);
        let fabric = FabricSpec::from_preset(preset);
        let mut sim = ClothSimSession::new(&pieces, &stitching, &m, &fabric, AvatarPose::APose);
        sim.step(frames);
        sim
    }

    #[test]
    fn test_lower_body_garments_hold_at_the_waist_in_every_fabric() {
        for preset in ["cotton_poplin", "jersey_knit", "silk_charmeuse", "denim_12oz"] {
            let sim = drape("aline_skirt", preset, 120);
            let max_y = sim
                .flat_positions
                .chunks_exact(3)
                .map(|c| c[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                max_y > sim.avatar.waist_y - 0.04,
                "skirt in {preset} slid down: top at {max_y:.3} m, waist at {:.3} m",
                sim.avatar.waist_y
            );
            assert!(sim.flat_positions.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn test_starter_styles_drape_without_poke_through_or_crumpling() {
        for (tmpl, preset) in [
            ("boxy_tee", "cotton_jersey_heavy"),
            ("boxy_tee", "silk_charmeuse"),
            ("wide_pleat_trouser", "wool_suiting"),
            ("barrel_trouser", "cotton_twill"),
        ] {
            let sim = drape(tmpl, preset, 120);
            let q = sim.drape_quality();
            let (_, max_gap) = sim.seam_gap_stats_mm();
            println!("[{tmpl}/{preset}] {q:?} max_gap={max_gap:.1}mm");
            // In A-pose the hands hang inside wide trouser legs; those finger points are a pose
            // limitation (the arms-raised pose clears them), so only tops must be fully clean.
            let poke_limit = if tmpl == "boxy_tee" { 0 } else { 100 };
            assert!(q.poke_through_count <= poke_limit, "{tmpl}/{preset} poke-through {q:?}");
            assert!(q.sharp_crease_pct < 5.0, "{tmpl}/{preset} crumpled {q:?}");
            assert!(max_gap < 25.0, "{tmpl}/{preset} seam open {max_gap:.1} mm");
            assert!(sim.flat_positions.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn test_fabric_properties_change_the_drape() {
        // Same tee: a crisp denim stands away from the body, a fluid silk falls onto it.
        let silk = drape("boxy_tee", "silk_charmeuse", 120).stats();
        let denim = drape("boxy_tee", "denim_12oz", 120).stats();
        assert!(
            denim.avg_clearance_mm > silk.avg_clearance_mm + 1.0,
            "denim clearance {:.1} mm vs silk {:.1} mm",
            denim.avg_clearance_mm,
            silk.avg_clearance_mm
        );
    }

    #[test]
    fn test_live_fabric_change_keeps_simulation_stable() {
        let mut sim = drape("aline_skirt", "cotton_poplin", 60);
        sim.set_fabric(FabricSpec::from_preset("jersey_knit"));
        sim.step(30);
        let mut f = FabricSpec::from_preset("silk_charmeuse");
        f.set_property("bend_warp_un_m", 0.1);
        f.set_property("stretch_weft_n_m", 10.0);
        sim.set_fabric(f);
        sim.step(30);
        assert!(sim.flat_positions.iter().all(|v| v.is_finite()));
        assert_eq!(sim.stats().fabric_name, "Silk Charmeuse (85 gsm)");
    }

    #[test]
    fn test_custom_style_split_seam_sews_tightly_in_3d() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let fabric = FabricSpec::from_preset("cotton_poplin");
        let ops = vec![ParametricOp::SplitStyleLine {
            id: det_id("op:3d_style_yoke"),
            piece_id: det_id("piece:bodice_back"),
            edge_a_idx: 2,
            t_a: 0.45,
            edge_b_idx: 5,
            t_b: 0.25,
            label_a: "Back Yoke".into(),
            label_b: "Lower Back".into(),
        }];
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &ops, &[]);
        assert_eq!(pieces.len(), 4);
        let mut sim = ClothSimSession::new(&pieces, &stitching, &m, &fabric, AvatarPose::APose);
        sim.step(40);
        let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
        assert!(
            avg_gap_mm < 2.5 && max_gap_mm < 8.0,
            "Style-split garment seams must zip tightly: avg={avg_gap_mm:.2}mm, max={max_gap_mm:.2}mm"
        );
    }

    #[test]
    fn test_adaptive_mesher_reduces_vertices_and_drapes_cleanly() {
        use crate::material::MaterialSet;
        use crate::xpbd::Sim3dOptions;
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        for (tmpl, preset) in [("bodice_sleeve", "cotton_poplin"), ("aline_skirt", "silk_charmeuse")] {
            let vars = build_default_variables(tmpl);
            let (pieces, stitching, _, _) = build_template_pieces(tmpl, &m, &e, &vars, &[], &[]);
            let mat = MaterialSet::single(&FabricSpec::from_preset(preset));
            let sim_auto = ClothSimSession::new_with_options(
                &pieces,
                &stitching,
                &m,
                &mat,
                AvatarPose::APose,
                AvatarBodyForm::Female,
                Sim3dOptions {
                    mesh_spacing_mm: 0.0,
                    ..Sim3dOptions::default()
                },
            );
            let sim_fine = ClothSimSession::new_with_options(
                &pieces,
                &stitching,
                &m,
                &mat,
                AvatarPose::APose,
                AvatarBodyForm::Female,
                Sim3dOptions {
                    mesh_spacing_mm: 7.0,
                    ..Sim3dOptions::default()
                },
            );
            let st = sim_auto.stats();
            let st_fine = sim_fine.stats();
            assert!(st.adaptive_mesh);
            assert!(
                st.min_edge_mm < st.max_edge_mm * 0.85,
                "{tmpl}: expected non-uniform edges, got {:.1}..{:.1} mm",
                st.min_edge_mm,
                st.max_edge_mm
            );
            assert!(
                st.vertex_count < st_fine.vertex_count,
                "{tmpl}: adaptive ({}) should have fewer verts than uniform 7mm ({})",
                st.vertex_count,
                st_fine.vertex_count
            );
            assert!(sim_auto.flat_positions.iter().all(|v| v.is_finite()));
            let q = sim_auto.drape_quality();
            assert!(q.poke_through_count <= 2, "{tmpl} adaptive poke-through {q:?}");
        }
    }

    #[test]
    fn test_interactive_grab_and_pin_moves_cloth() {
        let mut sim = drape("boxy_tee", "cotton_poplin", 20);
        let target_v = 10usize.min(sim.flat_positions.len() / 3 - 1);
        let orig = [
            sim.flat_positions[target_v * 3],
            sim.flat_positions[target_v * 3 + 1],
            sim.flat_positions[target_v * 3 + 2],
        ];
        assert!(sim.toggle_pin(target_v));
        assert_eq!(sim.stats().pin_count, 1);
        sim.step(10);
        let pinned_after = [
            sim.flat_positions[target_v * 3],
            sim.flat_positions[target_v * 3 + 1],
            sim.flat_positions[target_v * 3 + 2],
        ];
        let pin_drift = ((pinned_after[0] - orig[0]).powi(2)
            + (pinned_after[1] - orig[1]).powi(2)
            + (pinned_after[2] - orig[2]).powi(2))
        .sqrt();
        assert!(pin_drift < 0.005, "Pinned vertex drifted {pin_drift:.4} m");

        sim.clear_pins();
        assert_eq!(sim.stats().pin_count, 0);
        let pull_target = [orig[0] + 0.08, orig[1] + 0.04, orig[2] + 0.08];
        sim.begin_grab(target_v, pull_target, 0.12);
        sim.step(12);
        let grabbed = [
            sim.flat_positions[target_v * 3],
            sim.flat_positions[target_v * 3 + 1],
            sim.flat_positions[target_v * 3 + 2],
        ];
        let moved = ((grabbed[0] - orig[0]).powi(2)
            + (grabbed[1] - orig[1]).powi(2)
            + (grabbed[2] - orig[2]).powi(2))
        .sqrt();
        assert!(moved > 0.015, "Grab did not pull vertex enough: {moved:.4} m");

        // Aggressive grab stress: drag 0.80 m sideways, across torso, and into torso center;
        // seams must not tear, cloth must stay bounded, and body poke-through must remain zero.
        sim.move_grab([orig[0] + 0.80, orig[1], orig[2]]);
        sim.step(15);
        sim.move_grab([orig[0] - 0.80, orig[1], orig[2]]);
        sim.step(15);
        sim.move_grab([0.0, sim.avatar.waist_y + 0.05, 0.0]);
        sim.step(15);
        let dragged = [
            sim.flat_positions[target_v * 3],
            sim.flat_positions[target_v * 3 + 1],
            sim.flat_positions[target_v * 3 + 2],
        ];
        sim.end_grab();

        let drag_dist = ((dragged[0] - orig[0]).powi(2)
            + (dragged[1] - orig[1]).powi(2)
            + (dragged[2] - orig[2]).powi(2))
        .sqrt();
        assert!(drag_dist < 0.28, "Grab pulled vertex too far from origin: {drag_dist:.3} m");
        let (_avg_gap, max_gap_mm) = sim.seam_gap_stats_mm();
        assert!(max_gap_mm < 0.1, "Aggressive grab tore a seam: max_gap={max_gap_mm:.3} mm");
        let max_abs = sim.flat_positions.iter().copied().fold(0.0_f32, |a, b| a.max(b.abs()));
        assert!(
            sim.flat_positions.iter().all(|&v| v.is_finite()) && max_abs <= 2.2,
            "Aggressive grab pushed cloth out of bounds: max_abs={max_abs:.3}"
        );
        let q = sim.drape_quality();
        assert_eq!(q.poke_through_count, 0, "Aggressive grab clipped cloth into body: {q:?}");
    }

    #[test]
    fn test_avatar_anatomical_arms_hands_and_smooth_seams() {
        let tmpl = get_template();
        let m = BodyMeasurements::default();
        for form in [AvatarBodyForm::Female, AvatarBodyForm::Male] {
            for pose in [AvatarPose::APose, AvatarPose::ArmsRaised, AvatarPose::TPose] {
                let av = ParametricAvatar::from_measurements_with_form(&m, pose, form);
                let mesh = av.build_render_mesh();
                let vert_count = mesh.positions.len() / 3;
                assert_eq!(mesh.torso_mask.len(), vert_count);
                for i in 0..vert_count {
                    if tmpl.region[i] == REG_TORSO {
                        assert_eq!(mesh.torso_mask[i], 1.0);
                    } else if tmpl.region[i] == REG_ARM_L {
                        assert_eq!(mesh.torso_mask[i], 0.0);
                    }
                }

                // 1. Unclipped sculpted hands & fingers past wrist
                let (shoulder, wrist) = av.arm_segment(1.0);
                let arm_axis = wrist.sub(shoulder).normalized();
                let mut max_hand_dist = 0.0_f32;
                let mut el_sum = Vec3::ZERO;
                let mut el_cnt = 0usize;
                for vi in 0..vert_count {
                    let p = Vec3::new(
                        mesh.positions[vi * 3],
                        mesh.positions[vi * 3 + 1],
                        mesh.positions[vi * 3 + 2],
                    );
                    if tmpl.region[vi] == REG_ARM_L {
                        let d_past = p.sub(wrist).dot(arm_axis);
                        if d_past > max_hand_dist {
                            max_hand_dist = d_past;
                        }
                    }
                    let (b0, b1) = (tmpl.bone0[vi], tmpl.bone1[vi]);
                    if (b0 == BONE_UPPER_ARM_L && b1 == BONE_FOREARM_L)
                        || (b0 == BONE_FOREARM_L && b1 == BONE_UPPER_ARM_L)
                    {
                        el_sum = el_sum.add(p);
                        el_cnt += 1;
                    }
                }
                assert!(
                    max_hand_dist >= 0.135,
                    "{form:?} {pose:?} fingers appear cut off past wrist: max_hand_dist={max_hand_dist:.4} m"
                );

                // 2. Natural anatomical elbow flexion (never inverted/backward)
                assert!(el_cnt > 10);
                let el_center = el_sum.scale(1.0 / (el_cnt as f32));
                let upper_dir = el_center.sub(shoulder).normalized();
                let fore_dir = wrist.sub(el_center).normalized();
                if pose == AvatarPose::APose {
                    assert!(
                        fore_dir.z >= upper_dir.z + 0.01,
                        "{form:?} {pose:?} elbow appears inverted: upper_z={:.4}, fore_z={:.4}",
                        upper_dir.z,
                        fore_dir.z
                    );
                } else {
                    assert!(
                        fore_dir.z >= upper_dir.z - 0.01,
                        "{form:?} {pose:?} elbow appears inverted: upper_z={:.4}, fore_z={:.4}",
                        upper_dir.z,
                        fore_dir.z
                    );
                }

                // 3. Smooth Torso<->Leg and Torso<->Arm region boundaries (no torn trenches or sharp steps)
                for q in &tmpl.quads {
                    for &(ia, ib) in &[(q[0], q[1]), (q[1], q[2]), (q[2], q[3]), (q[3], q[0])] {
                        let ua = ia as usize;
                        let ub = ib as usize;
                        let ra = tmpl.region[ua];
                        let rb = tmpl.region[ub];
                        let is_leg_seam = (ra == REG_TORSO && matches!(rb, REG_LEG_L | REG_LEG_R))
                            || (rb == REG_TORSO && matches!(ra, REG_LEG_L | REG_LEG_R));
                        let is_arm_seam = (ra == REG_TORSO && matches!(rb, REG_ARM_L | REG_ARM_R))
                            || (rb == REG_TORSO && matches!(ra, REG_ARM_L | REG_ARM_R));
                        if !is_leg_seam && !is_arm_seam {
                            continue;
                        }
                        let rest_len = tmpl.base_verts[ua].sub(tmpl.base_verts[ub]).len();
                        if rest_len < 1e-4 {
                            continue;
                        }
                        let pa = Vec3::new(
                            mesh.positions[ua * 3],
                            mesh.positions[ua * 3 + 1],
                            mesh.positions[ua * 3 + 2],
                        );
                        let pb = Vec3::new(
                            mesh.positions[ub * 3],
                            mesh.positions[ub * 3 + 1],
                            mesh.positions[ub * 3 + 2],
                        );
                        let deformed_len = pa.sub(pb).len();
                        let (bounds, min_len) = if is_leg_seam {
                            (0.44..=1.95, 0.002_f32)
                        } else if pose == AvatarPose::APose {
                            (0.50..=2.6, 0.002_f32)
                        } else {
                            (0.15..=7.2, 0.001_f32)
                        };
                        assert!(
                            deformed_len >= min_len,
                            "{form:?} {pose:?} collapsed seam edge ({ua},{ub}): len={deformed_len:.4}m"
                        );
                        let ratio = deformed_len / rest_len;
                        assert!(
                            bounds.contains(&ratio),
                            "{form:?} {pose:?} seam distortion at region ({ra}<->{rb}) edge ({ua},{ub}): ratio={ratio:.3}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_watertight_conformal_seams_adaptive_and_uniform() {
        use crate::material::MaterialSet;
        use crate::xpbd::Sim3dOptions;

        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let materials = MaterialSet::single(&FabricSpec::from_preset("cotton_poplin"));

        for (label, spacing_mm) in [("adaptive", 0.0_f64), ("uniform_20mm", 20.0_f64)] {
            let opts = Sim3dOptions {
                mesh_spacing_mm: spacing_mm,
                ..Sim3dOptions::default()
            };
            let mut sim = ClothSimSession::new_with_options(
                &pieces,
                &stitching,
                &m,
                &materials,
                AvatarPose::APose,
                AvatarBodyForm::Female,
                opts,
            );
            sim.step(60);
            let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
            let weld_count = sim.seam_weld_cluster_count();
            let j3 = sim.junction_cluster_count(3);
            let j4 = sim.junction_cluster_count(4);
            let max_cluster = sim.max_junction_cluster_size();
            let max_spread = sim.max_weld_spread_mm();
            let q = sim.drape_quality();
            println!(
                "[{label}] welds={weld_count} j3={j3} j4={j4} max_cluster={max_cluster} avg_gap={avg_gap_mm:.3}mm max_gap={max_gap_mm:.3}mm max_spread={max_spread:.3}mm {q:?}"
            );
            assert!(weld_count > 20, "{label}: expected > 20 weld clusters, got {weld_count}");
            assert!(j3 >= 4, "{label}: expected >= 4 3-way+ junctions, got {j3}");
            assert!(j4 >= 2, "{label}: expected >= 2 4-way underarm junctions, got {j4}");
            assert!(avg_gap_mm < 0.02, "{label}: avg seam gap {avg_gap_mm:.3} mm");
            assert!(max_gap_mm < 0.05, "{label}: max seam gap {max_gap_mm:.3} mm");
            assert!(max_spread < 0.05, "{label}: max weld spread {max_spread:.3} mm");
            assert!(q.poke_through_count <= 5, "{label}: poke-through {q:?}");
            assert!(q.sharp_crease_pct < 5.0, "{label}: sharp_crease_pct {q:?}");
            assert!(
                sim.stats().motion_mm_s < 15.0,
                "{label}: garment still vibrating ({:.1} mm/s)",
                sim.stats().motion_mm_s
            );
        }
    }

    #[test]
    fn test_3way_and_4way_junctions_crotch_and_style_split() {
        use crate::material::MaterialSet;
        use crate::xpbd::Sim3dOptions;

        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let materials = MaterialSet::single(&FabricSpec::from_preset("cotton_poplin"));
        let adaptive_opts = Sim3dOptions {
            mesh_spacing_mm: 0.0,
            ..Sim3dOptions::default()
        };

        // 1. Trouser block: 4-way crotch junction (Front L/R + Back L/R inseam/crotch/seat).
        {
            let vars = build_default_variables("trouser_block");
            let (pieces, stitching, _, _) =
                build_template_pieces("trouser_block", &m, &e, &vars, &[], &[]);
            let mut sim = ClothSimSession::new_with_options(
                &pieces,
                &stitching,
                &m,
                &materials,
                AvatarPose::APose,
                AvatarBodyForm::Female,
                adaptive_opts,
            );
            sim.step(60);
            let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
            let j4 = sim.junction_cluster_count(4);
            let max_spread = sim.max_weld_spread_mm();
            let q = sim.drape_quality();
            println!(
                "[trouser_4way] j4={j4} avg_gap={avg_gap_mm:.3}mm max_gap={max_gap_mm:.3}mm max_spread={max_spread:.3}mm {q:?}"
            );
            assert!(j4 >= 1, "trouser_block: expected >= 1 4-way crotch junction, got {j4}");
            assert!(max_gap_mm < 0.05, "trouser_block: max seam gap {max_gap_mm:.3} mm");
            assert!(max_spread < 0.05, "trouser_block: max weld spread {max_spread:.3} mm");
            assert!(q.sharp_crease_pct < 5.0, "trouser_block: sharp_crease_pct {q:?}");
        }

        // 2. Style-split bodice: 3-way T-junctions where Back Yoke + Lower Back meet Armhole/CB.
        {
            let vars = build_default_variables("bodice_sleeve");
            let ops = vec![ParametricOp::SplitStyleLine {
                id: det_id("op:3d_style_yoke_junc"),
                piece_id: det_id("piece:bodice_back"),
                edge_a_idx: 2,
                t_a: 0.45,
                edge_b_idx: 5,
                t_b: 0.25,
                label_a: "Back Yoke".into(),
                label_b: "Lower Back".into(),
            }];
            let (pieces, stitching, _, _) =
                build_template_pieces("bodice_sleeve", &m, &e, &vars, &ops, &[]);
            let mut sim = ClothSimSession::new_with_options(
                &pieces,
                &stitching,
                &m,
                &materials,
                AvatarPose::APose,
                AvatarBodyForm::Female,
                adaptive_opts,
            );
            sim.step(60);
            let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
            let j3 = sim.junction_cluster_count(3);
            let max_spread = sim.max_weld_spread_mm();
            let q = sim.drape_quality();
            println!(
                "[style_split_3way] j3={j3} avg_gap={avg_gap_mm:.3}mm max_gap={max_gap_mm:.3}mm max_spread={max_spread:.3}mm {q:?}"
            );
            assert!(j3 >= 4, "style_split: expected >= 4 3-way junctions, got {j3}");
            assert!(max_gap_mm < 0.05, "style_split: max seam gap {max_gap_mm:.3} mm");
            assert!(max_spread < 0.05, "style_split: max weld spread {max_spread:.3} mm");
            assert!(q.sharp_crease_pct < 5.0, "style_split: sharp_crease_pct {q:?}");
        }
    }

    #[test]
    fn test_jogger_pants_elastic_cuffs_waist_and_hem_drape() {
        use crate::material::MaterialSet;
        use crate::xpbd::Sim3dOptions;

        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("jogger_pants");
        let (pieces, stitching, _, errors) =
            build_template_pieces("jogger_pants", &m, &e, &vars, &[], &[]);
        assert!(errors.is_empty(), "jogger_pants DAG errors: {errors:?}");

        let mut materials = MaterialSet::single(&FabricSpec::from_preset("fleece_sweatshirt"));
        materials.roles.push(("rib".into(), FabricSpec::from_preset("rib_cuff_elastic")));

        let opts = Sim3dOptions {
            mesh_spacing_mm: 0.0,
            ..Sim3dOptions::default()
        };
        let mut sim = ClothSimSession::new_with_options(
            &pieces,
            &stitching,
            &m,
            &materials,
            AvatarPose::APose,
            AvatarBodyForm::Female,
            opts,
        );
        sim.step(150);
        let (avg_gap_mm, max_gap_mm) = sim.seam_gap_stats_mm();
        let (el_cur_mm, el_orig_mm) = sim.elastic_band_length_mm();
        let q = sim.drape_quality();
        let motion = sim.stats().motion_mm_s;
        let p99_motion = sim.p99_motion_mm_s();
        let max_motion = sim.max_motion_mm_s();
        let (w_cur, _w_rest) = sim.waist_length_mm();
        let avg_waist_circ = w_cur / 3.0;
        let rings = sim.elastic_ring_fit_stats();
        let y_spans = sim.elastic_ring_y_span_mm();
        println!(
            "[jogger_pants] avg_gap={avg_gap_mm:.3}mm max_gap={max_gap_mm:.3}mm elastic={el_cur_mm:.1}/{el_orig_mm:.1}mm avg_waist={avg_waist_circ:.1}/{:.1}mm rings={rings:?} y_spans={y_spans:?} motion={motion:.2}mm/s p99={p99_motion:.2}mm/s max={max_motion:.2}mm/s {q:?}",
            sim.avatar.measured.waist_mm
        );
        assert!(el_orig_mm > 500.0, "expected active elastic bands on jogger_pants");
        assert!(rings.len() >= 4, "expected >= 4 closed elastic rings, got {}", rings.len());
        let waist_rings: Vec<(usize, (f32, f32, f32, f32))> = rings
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, r)| r.0 > 500.0)
            .collect();
        assert!(!waist_rings.is_empty(), "expected waistband elastic rings, got {rings:?}");
        let min_waist_circ = waist_rings.iter().map(|(_, r)| r.0).fold(f32::INFINITY, f32::min);
        assert!(
            min_waist_circ <= sim.avatar.measured.waist_mm + 40.0,
            "jogger_pants top waistband ring dangling ({min_waist_circ:.1} vs body {:.1} mm), rings={rings:?}",
            sim.avatar.measured.waist_mm
        );
        for &(ri, (_cur_circ_mm, _target_circ_mm, mean_clear_mm, max_clear_mm)) in &waist_rings {
            assert!(
                mean_clear_mm < 10.0,
                "jogger_pants waistband mean clearance too large ({mean_clear_mm:.2} mm), rings={rings:?}"
            );
            assert!(
                max_clear_mm < 14.0,
                "jogger_pants waistband max clearance too large ({max_clear_mm:.2} mm), rings={rings:?}"
            );
            assert!(
                y_spans[ri] < 18.0,
                "jogger_pants waistband ring not straight/level (y_span={:.2} mm), y_spans={y_spans:?}",
                y_spans[ri]
            );
        }
        assert_eq!(sim.drawstring_count(), 1, "expected 1 waist drawstring on jogger_pants");
        assert!(max_gap_mm < 0.05, "jogger_pants seam gap {max_gap_mm:.3} mm");
        assert!(sim.max_weld_spread_mm() < 0.05);
        assert!(q.sharp_crease_pct < 3.0, "jogger_pants sharp_crease_pct {q:?}");
        assert!(q.poke_through_count == 0, "jogger_pants poke_through {q:?}");
        assert!(motion < 5.0, "jogger_pants still vibrating ({motion:.2} mm/s)");
        assert!(p99_motion < 35.0, "jogger_pants p99 still vibrating ({p99_motion:.2} mm/s)");
        let r_verts = sim.render_positions().len() / 3;
        assert!(sim.render_positions().len() > sim.flat_positions.len() * 2);
        assert!(sim.render_indices().iter().all(|&idx| (idx as usize) < r_verts));
        assert!(sim.render_positions().iter().all(|v| v.is_finite()));
        assert!(sim.render_normals().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_wide_pleat_trouser_zipper_button_and_belt_loops() {
        let mut sim = drape("wide_pleat_trouser", "wool_suiting", 120);
        assert_eq!(sim.hardware_counts(), (1, 1, 1));
        let r_verts = sim.render_positions().len() / 3;
        assert!(sim.render_indices().iter().all(|&idx| (idx as usize) < r_verts));
        assert!(sim.render_positions().iter().all(|v| v.is_finite()));
        assert!(sim.render_normals().iter().all(|v| v.is_finite()));

        sim.set_zipper_fraction(0.25);
        sim.step(20);
        assert!(sim.render_positions().iter().all(|v| v.is_finite()));
        assert!(sim.render_normals().iter().all(|v| v.is_finite()));
    }

    /// Drafts a template with some variables overridden; `ns` moves its IDs to a namespace so two
    /// garments drafted from the same template can share one scene.
    fn draft_variant(
        tmpl: &str,
        overrides: &[(&str, &str)],
        ns: Option<&str>,
    ) -> (Vec<modelist_core::model::PatternPiece>, modelist_core::model::StitchingGraph) {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let mut vars = build_default_variables(tmpl);
        for (k, v) in overrides {
            let var = vars.iter_mut().find(|x| x.name == *k).expect("variable");
            var.expr = (*v).to_string();
        }
        let (mut pieces, mut stitching, _, _) = build_template_pieces(tmpl, &m, &e, &vars, &[], &[]);
        if let Some(ns) = ns {
            modelist_core::model::remap_entity_ids(&mut pieces, &mut stitching.bindings, ns);
        }
        (pieces, stitching)
    }

    struct LayerRun {
        sim: ClothSimSession,
        quality: crate::diagnostics::DrapeQuality,
        crossings: usize,
    }

    fn run_layers(
        garments: &[(Vec<modelist_core::model::PatternPiece>, modelist_core::model::StitchingGraph, u8)],
        preset: &str,
        frames: u32,
    ) -> LayerRun {
        use crate::material::MaterialSet;
        let mut pieces = Vec::new();
        let mut layers = Vec::new();
        let mut stitching = modelist_core::model::StitchingGraph::default();
        for (p, s, l) in garments {
            layers.extend(std::iter::repeat(*l).take(p.len()));
            pieces.extend(p.iter().cloned());
            stitching.bindings.extend(s.bindings.iter().cloned());
        }
        let fabric = FabricSpec::from_preset(preset);
        let mut sim = ClothSimSession::new_layered(
            &pieces,
            &layers,
            &stitching,
            &BodyMeasurements::default(),
            &MaterialSet::single(&fabric),
            AvatarPose::APose,
            AvatarBodyForm::Female,
        );
        sim.step(frames);
        let quality = sim.drape_quality();
        let crossings = sim.layer_crossings();
        LayerRun { sim, quality, crossings }
    }

    /// Intersecting triangle pairs split into (inside layer 0, between layers, inside layer 1+).
    fn intersections_by_layer(sim: &ClothSimSession) -> (usize, usize, usize) {
        let tris = sim.triangle_indices();
        let lay = sim.vertex_layers();
        let mut out = (0, 0, 0);
        for (a, b) in crate::diagnostics::intersecting_pairs(&sim.flat_positions, tris) {
            let la = lay[tris[a as usize * 3] as usize];
            let lb = lay[tris[b as usize * 3] as usize];
            match (la.min(lb), la.max(lb)) {
                (0, 0) => out.0 += 1,
                (x, y) if x != y => out.1 += 1,
                _ => out.2 += 1,
            }
        }
        out
    }

    #[test]
    fn test_layered_tees_do_not_pass_through_each_other() {
        let inner = draft_variant("boxy_tee", &[("tee_chest_ease", "120")], None);
        let outer = draft_variant(
            "boxy_tee",
            &[("tee_chest_ease", "300"), ("tee_body_len", "back_waist_len + 180")],
            Some("outer"),
        );
        let preset = "cotton_jersey_heavy";
        let alone_in = run_layers(&[(inner.0.clone(), inner.1.clone(), 0)], preset, 60);
        let flat = run_layers(
            &[(inner.0.clone(), inner.1.clone(), 0), (outer.0.clone(), outer.1.clone(), 0)],
            preset,
            60,
        );
        let t0 = std::time::Instant::now();
        let layered = run_layers(
            &[(inner.0.clone(), inner.1.clone(), 0), (outer.0.clone(), outer.1.clone(), 1)],
            preset,
            0,
        );
        let dress_time = t0.elapsed();
        let mut layered = layered;
        layered.sim.step(60);
        layered.quality = layered.sim.drape_quality();
        layered.crossings = layered.sim.layer_crossings();
        let t1 = std::time::Instant::now();
        let _ = run_layers(&[(inner.0.clone(), inner.1.clone(), 0)], preset, 0);
        eprintln!("dressing time: two layers {:?}, inner tee alone {:?}", dress_time, t1.elapsed());
        let by_layer = intersections_by_layer(&layered.sim);
        let stats = layered.sim.stats();
        eprintln!(
            "tees: inner alone {} self-intersections | one layer {} | layered {} (inner {}, between {}, outer {}) | crossings {} of {} | poke {} creases {:.1}% strain p99 {:.1}%",
            alone_in.quality.self_intersections,
            flat.quality.self_intersections,
            layered.quality.self_intersections,
            by_layer.0,
            by_layer.1,
            by_layer.2,
            layered.crossings,
            stats.vertex_count,
            layered.quality.poke_through_count,
            layered.quality.sharp_crease_pct,
            stats.max_strain_pct,
        );
        if let Ok(dir) = std::env::var("MODELIST_LAYER_OBJ") {
            dump_obj(&layered.sim, &format!("{dir}/layered"));
            dump_obj(&flat.sim, &format!("{dir}/flat"));
        }
        assert!(layered.sim.flat_positions.iter().all(|v| v.is_finite()));
        assert!(stats.min_clearance_mm >= 0.0);
        // Stacked cloth stays in order: almost no vertex ends up under the layer below it.
        assert!(layered.crossings <= stats.vertex_count / 100, "crossings {}", layered.crossings);
        // The inner garment drapes as it does alone (lower layers have priority).
        assert!(by_layer.0 <= 2 * alone_in.quality.self_intersections.max(50));
        // Far fewer intersections than both garments simulated as one layer. What remains sits
        // at the armpits, where an inner sleeve meets an outer side panel side by side; that
        // needs general cloth-to-cloth collision, which single garments lack too.
        assert!(layered.quality.self_intersections * 3 < flat.quality.self_intersections);
        assert!(layered.quality.poke_through_count <= 5);
        assert!(layered.quality.sharp_crease_pct <= 5.0);
        assert!(stats.max_strain_pct <= 40.0);
    }

    #[test]
    fn test_tuck_in_is_the_layer_order() {
        let tee = draft_variant("boxy_tee", &[("tee_chest_ease", "120")], None);
        let trousers = draft_variant("wide_pleat_trouser", &[], Some("trousers"));
        let preset = "cotton_jersey_heavy";
        for (name, tee_layer, trouser_layer) in [("tucked", 0u8, 1u8), ("untucked", 1, 0)] {
            let run = run_layers(
                &[(tee.0.clone(), tee.1.clone(), tee_layer), (trousers.0.clone(), trousers.1.clone(), trouser_layer)],
                preset,
                60,
            );
            let stats = run.sim.stats();
            let by_layer = intersections_by_layer(&run.sim);
            // Where is the tee hem relative to the trouser waist? Mean distance from the body of
            // tee vs trouser vertices in the band just below the waist.
            let lay = run.sim.vertex_layers();
            let waist = run.sim.avatar.waist_y;
            let (mut tee_d, mut tee_n, mut tr_d, mut tr_n) = (0.0f32, 0usize, 0.0f32, 0usize);
            for (i, p) in run.sim.flat_positions.chunks_exact(3).enumerate() {
                if p[1] < waist - 0.10 || p[1] > waist - 0.03 {
                    continue;
                }
                let d = run.sim.flat_clearance_mm[i];
                if lay[i] == tee_layer {
                    tee_d += d;
                    tee_n += 1;
                } else {
                    tr_d += d;
                    tr_n += 1;
                }
            }
            let (tee_d, tr_d) = (tee_d / tee_n.max(1) as f32, tr_d / tr_n.max(1) as f32);
            eprintln!(
                "{name}: intersections {} (between layers {}) crossings {} poke {} strain p99 {:.1}% | below waist: tee {tee_d:.1} mm, trousers {tr_d:.1} mm from the body",
                run.quality.self_intersections, by_layer.1, run.crossings, run.quality.poke_through_count, stats.max_strain_pct
            );
            if let Ok(dir) = std::env::var("MODELIST_LAYER_OBJ") {
                dump_obj(&run.sim, &format!("{dir}/{name}"));
            }
            assert!(run.sim.flat_positions.iter().all(|v| v.is_finite()));
            assert!(run.crossings <= stats.vertex_count / 100, "{name} crossings {}", run.crossings);
            if tee_n > 20 && tr_n > 20 {
                if tee_layer == 0 {
                    assert!(tee_d < tr_d, "tucked: the tee should sit under the trousers");
                } else {
                    assert!(tee_d > tr_d, "untucked: the tee should hang over the trousers");
                }
            }
        }
    }

    /// Writes `<base>_avatar.obj` and `<base>_cloth.obj` (5th vertex value = layer * 10).
    fn dump_obj(sim: &ClothSimSession, base: &str) {
        use std::fmt::Write as _;
        let mut a = String::new();
        let am = &sim.avatar_mesh;
        for p in am.positions.chunks_exact(3) {
            let _ = writeln!(a, "v {} {} {}", p[0], p[1], p[2]);
        }
        for f in am.indices.chunks_exact(3) {
            let _ = writeln!(a, "f {} {} {}", f[0] + 1, f[1] + 1, f[2] + 1);
        }
        std::fs::write(format!("{base}_avatar.obj"), a).unwrap();
        let mut c = String::new();
        let lay = sim.vertex_layers();
        for (i, p) in sim.flat_positions.chunks_exact(3).enumerate() {
            let _ = writeln!(c, "v {} {} {} {}", p[0], p[1], p[2], lay.get(i).copied().unwrap_or(0) as f32 * 10.0);
        }
        for f in sim.triangle_indices().chunks_exact(3) {
            let _ = writeln!(c, "f {} {} {}", f[0] + 1, f[1] + 1, f[2] + 1);
        }
        std::fs::write(format!("{base}_cloth.obj"), c).unwrap();
    }
}
