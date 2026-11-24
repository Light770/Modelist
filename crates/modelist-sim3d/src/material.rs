//! Fabric -> solver material conversion.
//!
//! [`FabricSpec`] stores measurable quantities (N/m, uN*m, g/m2, %). This module turns them into
//! the SI parameters the XPBD solver consumes, and resolves which fabric each piece uses from its
//! `fabric_role` (shell, rib, lining, interfaced shell...).

use modelist_core::model::{fabric_role_is_interfaced, fabric_role_key, FabricSpec, PatternProject};

/// Fabrics available to one simulation: the shell plus optional per-role overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialSet {
    pub shell: FabricSpec,
    /// `(role_key, fabric)` pairs; keys come from [`fabric_role_key`].
    pub roles: Vec<(String, FabricSpec)>,
}

impl MaterialSet {
    pub fn single(shell: &FabricSpec) -> Self {
        Self {
            shell: shell.clone().normalized(),
            roles: Vec::new(),
        }
    }

    pub fn from_project(project: &PatternProject) -> Self {
        Self {
            shell: project.fabric.clone().normalized(),
            roles: project
                .role_fabrics
                .iter()
                .map(|rf| (fabric_role_key(&rf.role), rf.fabric.clone().normalized()))
                .collect(),
        }
    }

    pub fn fabric_for_role(&self, role: &str) -> &FabricSpec {
        let key = fabric_role_key(role);
        if key != "shell" {
            if let Some((_, f)) = self.roles.iter().find(|(k, _)| *k == key) {
                return f;
            }
        }
        &self.shell
    }

    /// Solver material for a piece role (applies the interfacing modifier when needed).
    pub(crate) fn solver_material(&self, role: &str) -> SolverMaterial {
        SolverMaterial::from_spec(self.fabric_for_role(role), fabric_role_is_interfaced(role))
    }
}

/// Solver-ready material, SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SolverMaterial {
    /// kg/m2
    pub density: f32,
    /// m
    pub thickness: f32,
    /// Tensile stiffness along warp / weft, N/m.
    pub e_warp: f32,
    pub e_weft: f32,
    /// In-plane shear stiffness, N/m.
    pub g_shear: f32,
    /// Bending rigidity with warp / weft yarns bent, N*m.
    pub b_warp: f32,
    pub b_weft: f32,
    /// Strain at which yarns lock (fraction).
    pub lock_warp: f32,
    pub lock_weft: f32,
    /// Compression stiffness relative to tension.
    pub buckling: f32,
    pub friction: f32,
    /// Velocity decay rate, 1/s.
    pub damping_rate: f32,
    /// Rest-length factors after shrinkage (1 = none).
    pub rest_warp: f32,
    pub rest_weft: f32,
}

/// Stiffening factor once the strain passes the lock point (yarn jamming).
pub(crate) const LOCK_STIFFENING: f32 = 40.0;

impl SolverMaterial {
    pub fn from_spec(spec: &FabricSpec, interfaced: bool) -> Self {
        let f = spec.clone().normalized();
        let mut m = Self {
            density: (f.density_gsm * 1e-3) as f32,
            thickness: (f.thickness_mm * 1e-3) as f32,
            e_warp: f.stretch_warp_n_m as f32,
            e_weft: f.stretch_weft_n_m as f32,
            g_shear: f.shear_n_m as f32,
            b_warp: (f.bend_warp_un_m * 1e-6) as f32,
            b_weft: (f.bend_weft_un_m * 1e-6) as f32,
            lock_warp: (f.max_stretch_warp_pct * 0.01) as f32,
            lock_weft: (f.max_stretch_weft_pct * 0.01) as f32,
            buckling: f.buckling_ratio as f32,
            friction: f.friction as f32,
            damping_rate: 0.8 + 7.0 * f.damping as f32,
            rest_warp: (1.0 - f.shrinkage_warp_pct * 0.01).clamp(0.85, 1.05) as f32,
            rest_weft: (1.0 - f.shrinkage_weft_pct * 0.01).clamp(0.85, 1.05) as f32,
        };
        if interfaced {
            // Fusible interfacing: a stiff, light woven layer bonded to the shell.
            m.density += 0.030;
            m.thickness += 0.0003;
            m.e_warp = m.e_warp * 3.0 + 3000.0;
            m.e_weft = m.e_weft * 3.0 + 3000.0;
            m.g_shear = m.g_shear * 3.0 + 60.0;
            m.b_warp = m.b_warp * 5.0 + 20e-6;
            m.b_weft = m.b_weft * 5.0 + 20e-6;
            m.lock_warp = m.lock_warp.min(0.015);
            m.lock_weft = m.lock_weft.min(0.015);
            m.buckling = m.buckling.max(0.6);
        }
        m
    }

    /// Distance the cloth keeps from the body surface (half thickness plus a skin gap), m.
    pub fn contact_offset(&self) -> f32 {
        0.0022 + 0.6 * self.thickness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interfacing_stiffens_and_locks() {
        let f = FabricSpec::from_preset("jersey_knit");
        let plain = SolverMaterial::from_spec(&f, false);
        let inter = SolverMaterial::from_spec(&f, true);
        assert!(inter.b_warp > plain.b_warp * 4.0);
        assert!(inter.e_weft > plain.e_weft * 3.0);
        assert!(inter.lock_weft <= 0.015 + 1e-6);
    }

    #[test]
    fn roles_resolve_with_fallback() {
        let mut set = MaterialSet::single(&FabricSpec::from_preset("cotton_jersey_heavy"));
        set.roles
            .push(("rib".into(), FabricSpec::from_preset("rib_knit")));
        assert_eq!(set.fabric_for_role("Rib").id, "rib_knit");
        assert_eq!(set.fabric_for_role("Shell (interfaced)").id, "cotton_jersey_heavy");
        assert_eq!(set.fabric_for_role("Lining").id, "cotton_jersey_heavy");
    }
}
