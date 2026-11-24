use crate::avatar::Vec3;
use std::sync::OnceLock;

static MANNEQUIN_BYTES: &[u8] = include_bytes!("../assets/mannequin_cc0.bin");
static TEMPLATE_CACHE: OnceLock<MannequinTemplate> = OnceLock::new();

pub const BONE_TORSO: u8 = 0;
pub const BONE_CLAVICLE_L: u8 = 1;
pub const BONE_UPPER_ARM_L: u8 = 2;
pub const BONE_FOREARM_L: u8 = 3;
pub const BONE_CLAVICLE_R: u8 = 4;
pub const BONE_UPPER_ARM_R: u8 = 5;
pub const BONE_FOREARM_R: u8 = 6;
pub const BONE_THIGH_L: u8 = 7;
pub const BONE_CALF_L: u8 = 8;
pub const BONE_THIGH_R: u8 = 9;
pub const BONE_CALF_R: u8 = 10;

pub const REG_TORSO: u8 = 0;
pub const REG_HEAD: u8 = 1;
pub const REG_ARM_L: u8 = 2;
pub const REG_ARM_R: u8 = 3;
pub const REG_LEG_L: u8 = 4;
pub const REG_LEG_R: u8 = 5;

pub const J_PELVIS: usize = 0;
pub const J_CROTCH: usize = 1;
pub const J_WAIST: usize = 2;
pub const J_BUST: usize = 3;
pub const J_NECK_BASE: usize = 4;
pub const J_HEAD_TOP: usize = 5;
pub const J_SHOULDER_L: usize = 6;
pub const J_ELBOW_L: usize = 7;
pub const J_WRIST_L: usize = 8;
pub const J_HAND_L: usize = 9;
pub const J_HIP_L: usize = 10;
pub const J_KNEE_L: usize = 11;
pub const J_ANKLE_L: usize = 12;
pub const J_FOOT_L: usize = 13;

pub const MORPH_MALE_FORM: usize = 0;
pub const MORPH_WEIGHT_INCR: usize = 1;
pub const MORPH_WEIGHT_DECR: usize = 2;
pub const MORPH_BUST_INCR: usize = 3;
pub const MORPH_BUST_DECR: usize = 4;
pub const MORPH_WAIST_INCR: usize = 5;
pub const MORPH_WAIST_DECR: usize = 6;
pub const MORPH_HIPS_INCR: usize = 7;
pub const MORPH_HIPS_DECR: usize = 8;
pub const MORPH_SHOULDER_INCR: usize = 9;
pub const MORPH_SHOULDER_DECR: usize = 10;
pub const MORPH_UPPERARM_INCR: usize = 11;
pub const MORPH_UPPERARM_DECR: usize = 12;
pub const MORPH_WRIST_INCR: usize = 13;
pub const MORPH_WRIST_DECR: usize = 14;
pub const MORPH_THIGH_INCR: usize = 15;
pub const MORPH_THIGH_DECR: usize = 16;
pub const MORPH_KNEE_INCR: usize = 17;
pub const MORPH_KNEE_DECR: usize = 18;
pub const MORPH_ANKLE_INCR: usize = 19;
pub const MORPH_ANKLE_DECR: usize = 20;

#[derive(Debug, Clone, Copy)]
pub struct MorphDelta {
    pub vid: u16,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
}

#[derive(Debug, Clone)]
pub struct MannequinTemplate {
    pub src_joints: [Vec3; 14],
    pub src_crotch_y: f32,
    pub src_hip_y: f32,
    pub src_waist_y: f32,
    pub src_bust_y: f32,
    pub src_shoulder_y: f32,
    pub src_neck_y: f32,
    pub src_ankle_y: f32,
    pub src_knee_y: f32,
    pub src_head_top_y: f32,
    pub src_shoulder_half_x: f32,
    pub base_verts: Vec<Vec3>,
    pub bone0: Vec<u8>,
    pub bone1: Vec<u8>,
    pub weight0: Vec<f32>,
    pub region: Vec<u8>,
    pub quads: Vec<[u16; 4]>,
    pub morphs: Vec<Vec<MorphDelta>>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        s
    }
    fn u16(&mut self) -> u16 {
        let b = self.take(2);
        u16::from_le_bytes([b[0], b[1]])
    }
    fn i16(&mut self) -> i16 {
        let b = self.take(2);
        i16::from_le_bytes([b[0], b[1]])
    }
    fn u32(&mut self) -> u32 {
        let b = self.take(4);
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
    fn f32(&mut self) -> f32 {
        let b = self.take(4);
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
}

pub fn get_template() -> &'static MannequinTemplate {
    TEMPLATE_CACHE.get_or_init(parse_template)
}

fn parse_template() -> MannequinTemplate {
    let mut r = Reader {
        bytes: MANNEQUIN_BYTES,
        pos: 0,
    };
    let magic = r.take(4);
    assert_eq!(magic, b"MLMN", "Corrupt mannequin asset magic");
    let ver = r.u16();
    assert_eq!(ver, 1, "Unsupported mannequin asset version {ver}");
    let vert_count = r.u32() as usize;
    let quad_count = r.u32() as usize;
    let target_count = r.u16() as usize;

    let mut src_joints = [Vec3::ZERO; 14];
    for j in &mut src_joints {
        *j = Vec3::new(r.f32(), r.f32(), r.f32());
    }
    let src_crotch_y = r.f32();
    let src_hip_y = r.f32();
    let src_waist_y = r.f32();
    let src_bust_y = r.f32();
    let src_shoulder_y = r.f32();
    let src_neck_y = r.f32();
    let src_ankle_y = r.f32();
    let src_knee_y = r.f32();
    let src_head_top_y = r.f32();
    let src_shoulder_half_x = r.f32();

    let mut base_verts = Vec::with_capacity(vert_count);
    for _ in 0..vert_count {
        base_verts.push(Vec3::new(
            (r.i16() as f32) * 0.0001,
            (r.i16() as f32) * 0.0001,
            (r.i16() as f32) * 0.0001,
        ));
    }

    let mut bone0 = Vec::with_capacity(vert_count);
    let mut bone1 = Vec::with_capacity(vert_count);
    let mut weight0 = Vec::with_capacity(vert_count);
    let mut region = Vec::with_capacity(vert_count);
    let skin = r.take(vert_count * 4);
    for chunk in skin.chunks_exact(4) {
        bone0.push(chunk[0]);
        bone1.push(chunk[1]);
        weight0.push((chunk[2] as f32) / 255.0);
        region.push(chunk[3]);
    }

    let mut quads = Vec::with_capacity(quad_count);
    for _ in 0..quad_count {
        quads.push([r.u16(), r.u16(), r.u16(), r.u16()]);
    }

    let mut morphs = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        let dc = r.u32() as usize;
        let mut list = Vec::with_capacity(dc);
        for _ in 0..dc {
            list.push(MorphDelta {
                vid: r.u16(),
                dx: (r.i16() as f32) * 0.0001,
                dy: (r.i16() as f32) * 0.0001,
                dz: (r.i16() as f32) * 0.0001,
            });
        }
        morphs.push(list);
    }

    MannequinTemplate {
        src_joints,
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
        base_verts,
        bone0,
        bone1,
        weight0,
        region,
        quads,
        morphs,
    }
}
