//! Blender-generated models (`tools/blender/build_models.py`) with photo-based PBR materials.
//!
//! * `assets/models/<key>.glb`: one mesh per material slot, named `<key>_<Slot>`, already in the
//!   game's frame (+Y up, −Z forward); the second UV map belongs to `<key>_ao.png`, the baked
//!   ambient occlusion.
//! * `assets/models/materials.ron`: slot → texture set, tint, roughness/metalness factors,
//!   emissive, team colour (written by the generator from `cc_kit.MATERIALS`).
//! * `assets/textures/<id>/{albedo,normal,arm}.jpg`: CC0 texture sets (`tools/fetch_textures.py`,
//!   credits in `assets/textures/CREDITS.md`).
//!
//! Bevy does not create mipmaps for JPG/PNG files and tiled photo textures shimmer without them,
//! so the images are decoded, resized and mip-mapped on a background thread (as in MMM). When
//! everything is there the parts in `Models` are replaced and `Models::ready` is set; models
//! without a GLB keep their primitive fallback.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::image::{CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor, ImageType};
use bevy::mesh::UvChannel;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use cc_core::{PLAYER_COLORS, Res as ResKind};
use crossbeam_channel::Receiver;
use serde::Deserialize;

use super::models::{Models, Part, team_color, team_glow_color};
use crate::Content;

/// Model keys of the resource deposits.
pub const DEPOSIT_KEYS: [(ResKind, &str); 3] =
    [(ResKind::Metal, "deposit_metal"), (ResKind::Crystal, "deposit_crystal"), (ResKind::Gas, "deposit_gas")];

/// Texture edge length in pixels (the files are 1024 px). `CC_TEXTURE_SIZE=1024` for close-ups.
fn texture_size() -> u32 {
    std::env::var("CC_TEXTURE_SIZE").ok().and_then(|v| v.parse::<u32>().ok()).filter(|s| s.is_power_of_two()).unwrap_or(512).clamp(64, 2048)
}

/// One material slot (see `cc_kit.MATERIALS`).
#[derive(Deserialize, Clone, Debug)]
pub struct SlotMat {
    pub tex: Option<String>,
    pub tint: (u8, u8, u8),
    pub rough: f32,
    pub metal: f32,
    pub reflectance: f32,
    pub emissive: (f32, f32, f32),
    pub team: bool,
    pub team_glow: bool,
    pub alpha: f32,
    pub additive: bool,
}

/// The directory Bevy loads assets from.
pub fn asset_root() -> PathBuf {
    bevy::asset::io::file::FileAssetReader::get_base_path().join("assets")
}

pub fn read_slot_table(root: &Path) -> Option<HashMap<String, SlotMat>> {
    let text = std::fs::read_to_string(root.join("models/materials.ron")).ok()?;
    match ron::from_str(&text) {
        Ok(t) => Some(t),
        Err(e) => {
            warn!("models/materials.ron: {e}");
            None
        }
    }
}

struct TextureSet {
    albedo: Image,
    normal: Image,
    arm: Image,
}

struct Decoded {
    textures: HashMap<String, TextureSet>,
    ao: HashMap<String, Image>,
}

#[derive(Resource)]
pub struct ModelLoad {
    slots: HashMap<String, SlotMat>,
    gltfs: Vec<(String, Handle<Gltf>)>,
    rx: Option<Receiver<Decoded>>,
    decoded: Option<Decoded>,
}

/// Every model key the game can show: units, buildings and deposits.
fn model_keys(content: &cc_core::ContentDb) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = content.units.iter().map(|u| u.model.clone()).collect();
    keys.extend(content.buildings.iter().map(|b| b.model.clone()));
    keys.extend(DEPOSIT_KEYS.iter().map(|(_, k)| k.to_string()));
    keys
}

/// Start-up: starts loading the GLBs and decoding the textures (nothing to do without files).
pub fn start_model_load(mut commands: Commands, server: Res<AssetServer>, content: Res<Content>) {
    let root = asset_root();
    let Some(slots) = read_slot_table(&root) else {
        info!("no Blender models found under {root:?}; using primitive models");
        commands.insert_resource(ModelLoad { slots: HashMap::new(), gltfs: Vec::new(), rx: None, decoded: None });
        return;
    };
    let keys: Vec<String> = model_keys(&content.0).into_iter().filter(|k| root.join(format!("models/{k}.glb")).exists()).collect();
    let gltfs = keys.iter().map(|k| (k.clone(), server.load(format!("models/{k}.glb")))).collect();
    let textures: BTreeSet<String> = slots.values().filter_map(|s| s.tex.clone()).collect();
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::Builder::new()
        .name("model-textures".into())
        .spawn(move || {
            let t0 = std::time::Instant::now();
            let size = texture_size();
            let mut out = Decoded { textures: HashMap::new(), ao: HashMap::new() };
            for id in textures {
                let dir = root.join("textures").join(&id);
                let albedo = load_image(&dir.join("albedo.jpg"), Some(size), Kind::Albedo, true);
                let normal = load_image(&dir.join("normal.jpg"), Some(size), Kind::Normal, true);
                let arm = load_image(&dir.join("arm.jpg"), Some(size), Kind::Linear, true);
                match (albedo, normal, arm) {
                    (Some(albedo), Some(normal), Some(arm)) => {
                        out.textures.insert(id, TextureSet { albedo, normal, arm });
                    }
                    _ => warn!("texture set {id} is incomplete (run tools/fetch_textures.py)"),
                }
            }
            for k in keys {
                if let Some(img) = load_image(&root.join(format!("models/{k}_ao.png")), None, Kind::Gray, false) {
                    out.ao.insert(k, img);
                }
            }
            info!("model textures decoded in {:.2}s", t0.elapsed().as_secs_f32());
            let _ = tx.send(out);
        })
        .expect("spawn texture decoder");
    commands.insert_resource(ModelLoad { slots, gltfs, rx: Some(rx), decoded: None });
}

/// Replaces the primitive models once the GLBs and textures are ready.
#[allow(clippy::too_many_arguments)]
pub fn finish_model_load(
    mut commands: Commands,
    load: Option<ResMut<ModelLoad>>,
    models: Option<ResMut<Models>>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gmeshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(mut load), Some(mut models)) = (load, models) else { return };
    if let Some(rx) = &load.rx {
        match rx.try_recv() {
            Ok(d) => {
                load.decoded = Some(d);
                load.rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => return,
            Err(crossbeam_channel::TryRecvError::Disconnected) => load.rx = None,
        }
    }
    let pending = |h: &Handle<Gltf>| !server.is_loaded_with_dependencies(h.id()) && !matches!(server.load_state(h.id()), LoadState::Failed(_));
    if load.gltfs.iter().any(|(_, h)| pending(h)) {
        return;
    }
    let decoded = load.decoded.take().unwrap_or(Decoded { textures: HashMap::new(), ao: HashMap::new() });
    let textures: HashMap<String, [Handle<Image>; 3]> = decoded
        .textures
        .into_iter()
        .map(|(id, t)| (id, [images.add(t.albedo), images.add(t.normal), images.add(t.arm)]))
        .collect();
    let ao: HashMap<String, Handle<Image>> = decoded.ao.into_iter().map(|(k, img)| (k, images.add(img))).collect();
    let mut count = 0;
    for (key, handle) in &load.gltfs {
        let Some(gltf) = gltfs.get(handle) else {
            warn!("models/{key}.glb failed to load; keeping the primitive model");
            continue;
        };
        let mut parts = Vec::new();
        for gm in gltf.meshes.iter().filter_map(|h| gmeshes.get(h)) {
            let slot = gm.name.rsplit('_').next().unwrap_or("Hull");
            let spec = load.slots.get(slot).or_else(|| load.slots.get("Hull"));
            let Some(spec) = spec else { continue };
            let mats = slot_materials(&mut materials, spec, textures_of(spec, &textures), ao.get(key));
            for prim in &gm.primitives {
                if let Some(mut mesh) = meshes.get_mut(&prim.mesh)
                    && mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none()
                    && mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some()
                    && let Err(e) = mesh.generate_tangents()
                {
                    warn!("{key}: no tangents for {}: {e}", gm.name);
                }
                parts.push(Part { mesh: prim.mesh.clone(), mats: mats.clone(), transform: Transform::IDENTITY });
            }
        }
        if parts.is_empty() {
            continue;
        }
        match DEPOSIT_KEYS.iter().find(|(_, k)| k == key) {
            Some((r, _)) => models.set_deposit(*r, parts),
            None => models.set_model(key, parts),
        }
        count += 1;
    }
    if count > 0 {
        info!("{count} Blender models with {} texture sets in use", textures.len());
    }
    models.ready = true;
    commands.remove_resource::<ModelLoad>();
}

fn textures_of<'a>(spec: &SlotMat, textures: &'a HashMap<String, [Handle<Image>; 3]>) -> Option<&'a [Handle<Image>; 3]> {
    spec.tex.as_ref().and_then(|t| textures.get(t))
}

/// The material(s) of one slot of one model: one per player colour for team slots.
fn slot_materials(
    materials: &mut Assets<StandardMaterial>,
    spec: &SlotMat,
    tex: Option<&[Handle<Image>; 3]>,
    ao: Option<&Handle<Image>>,
) -> Vec<Handle<StandardMaterial>> {
    let teams = if spec.team || spec.team_glow { PLAYER_COLORS.len() } else { 1 };
    (0..teams as u8)
        .map(|team| {
            let (r, g, b) = spec.tint;
            let mut base = Color::srgb_u8(r, g, b);
            if spec.team {
                base = team_color(team);
            }
            let (er, eg, eb) = spec.emissive;
            let emissive = if spec.team_glow { team_glow_color(team) } else { LinearRgba::rgb(er, eg, eb) };
            let mut m = StandardMaterial {
                base_color: base,
                perceptual_roughness: spec.rough,
                metallic: spec.metal,
                reflectance: spec.reflectance,
                emissive,
                ..default()
            };
            if let Some([albedo, normal, arm]) = tex {
                m.base_color_texture = Some(albedo.clone());
                m.normal_map_texture = Some(normal.clone());
                m.metallic_roughness_texture = Some(arm.clone());
            }
            if let Some(ao) = ao {
                m.occlusion_texture = Some(ao.clone());
                m.occlusion_channel = UvChannel::Uv1;
            }
            if spec.additive {
                m.base_color = base.with_alpha(spec.alpha);
                m.alpha_mode = AlphaMode::Add;
                m.unlit = true;
                m.occlusion_texture = None;
            }
            materials.add(m)
        })
        .collect()
}

// ------------------------------------------------------------------------------- image decoding

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// sRGB colour: averaged in linear space.
    Albedo,
    /// Tangent-space normal: averaged and renormalised.
    Normal,
    /// Linear data (roughness, metalness, AO).
    Linear,
    /// One linear channel (baked AO maps), stored as R8.
    Gray,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(v: f32) -> u8 {
    let c = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Decodes an image file into `channels`-byte pixels, resized (nearest) to `size` if given.
fn decode(path: &Path, size: Option<u32>, channels: usize) -> Option<(u32, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?;
    let img = Image::from_buffer(&bytes, ImageType::Extension(ext), CompressedImageFormats::NONE, false, ImageSampler::Default, RenderAssetUsages::RENDER_WORLD)
        .map_err(|e| warn!("{path:?}: {e}"))
        .ok()?;
    let (w, h) = (img.width(), img.height());
    let data = img.data.as_ref()?;
    let src_ch = match img.texture_descriptor.format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => 4,
        TextureFormat::Rg8Unorm => 2,
        TextureFormat::R8Unorm => 1,
        other => {
            warn!("{path:?}: unsupported pixel format {other:?}");
            return None;
        }
    };
    if data.len() != (w * h) as usize * src_ch || w != h || !w.is_power_of_two() {
        warn!("{path:?}: expected a square power-of-two image");
        return None;
    }
    let out_size = size.unwrap_or(w).min(w);
    let step = w / out_size;
    let mut out = Vec::with_capacity((out_size * out_size) as usize * channels);
    for y in 0..out_size {
        for x in 0..out_size {
            let i = (((y * step) * w + x * step) as usize) * src_ch;
            for c in 0..channels {
                let v = match src_ch {
                    1 | 2 => if c == 3 { 255 } else { data[i] },
                    _ => data[i + c.min(3)],
                };
                out.push(v);
            }
        }
    }
    Some((out_size, out))
}

/// Box-filtered mip chain (correct for colour, normals and data).
fn mip_chain(level0: Vec<u8>, size: u32, channels: usize, kind: Kind) -> Vec<Vec<u8>> {
    let mut levels = vec![level0];
    let mut s = size as usize;
    while s > 1 {
        let src = levels.last().expect("level");
        let n = s / 2;
        let mut out = vec![0u8; n * n * channels];
        for y in 0..n {
            for x in 0..n {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = ((y * 2 + dy) * s + x * 2 + dx) * channels;
                    for c in 0..channels {
                        let v = src[i + c] as f32 / 255.0;
                        acc[c] += match kind {
                            Kind::Albedo if c < 3 => srgb_to_linear(v),
                            Kind::Normal if c < 3 => v * 2.0 - 1.0,
                            _ => v,
                        };
                    }
                }
                let o = (y * n + x) * channels;
                match kind {
                    Kind::Normal => {
                        let v = Vec3::new(acc[0], acc[1], acc[2]).normalize_or(Vec3::Z);
                        out[o] = ((v.x * 0.5 + 0.5) * 255.0 + 0.5) as u8;
                        out[o + 1] = ((v.y * 0.5 + 0.5) * 255.0 + 0.5) as u8;
                        out[o + 2] = ((v.z * 0.5 + 0.5) * 255.0 + 0.5) as u8;
                        out[o + 3] = 255;
                    }
                    _ => {
                        for c in 0..channels {
                            let a = acc[c] / 4.0;
                            out[o + c] = if kind == Kind::Albedo && c < 3 { linear_to_srgb(a) } else { (a * 255.0 + 0.5) as u8 };
                        }
                    }
                }
            }
        }
        levels.push(out);
        s = n;
    }
    levels
}

fn load_image(path: &Path, size: Option<u32>, kind: Kind, repeat: bool) -> Option<Image> {
    let channels = if kind == Kind::Gray { 1 } else { 4 };
    let (size, level0) = decode(path, size, channels)?;
    let levels = mip_chain(level0, size, channels, kind);
    let format = match kind {
        Kind::Albedo => TextureFormat::Rgba8UnormSrgb,
        Kind::Gray => TextureFormat::R8Unorm,
        _ => TextureFormat::Rgba8Unorm,
    };
    let mut img = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        levels[0].clone(),
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    // `Image::new` validates one level; attach the whole chain afterwards (as in MMM).
    img.texture_descriptor.mip_level_count = levels.len() as u32;
    img.data = Some(levels.concat());
    let address = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: if repeat { 8 } else { 1 },
        ..default()
    });
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    }

    /// Mesh/node names inside a GLB's JSON chunk.
    fn glb_names(path: &Path) -> Vec<String> {
        let b = std::fs::read(path).expect("glb");
        assert_eq!(&b[0..4], b"glTF");
        let n = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
        let json = std::str::from_utf8(&b[20..20 + n]).expect("json");
        json.split("\"name\":\"").skip(1).filter_map(|s| s.split('"').next()).map(str::to_string).collect()
    }

    #[test]
    fn every_model_has_a_glb_with_known_slots() {
        let root = root();
        let slots = read_slot_table(&root).expect("models/materials.ron");
        for s in slots.values() {
            if let Some(t) = &s.tex {
                for f in ["albedo.jpg", "normal.jpg", "arm.jpg"] {
                    assert!(root.join("textures").join(t).join(f).exists(), "missing textures/{t}/{f}");
                }
            }
        }
        let content = cc_core::ContentDb::embedded();
        for key in model_keys(&content) {
            let glb = root.join(format!("models/{key}.glb"));
            assert!(glb.exists(), "missing models/{key}.glb (run tools/blender/build_models.py)");
            assert!(root.join(format!("models/{key}_ao.png")).exists(), "missing models/{key}_ao.png");
            let names = glb_names(&glb);
            let mut found = 0;
            for name in names.iter().filter(|n| n.starts_with(&format!("{key}_"))) {
                let slot = name.rsplit('_').next().unwrap_or_default();
                assert!(slots.contains_key(slot), "{key}: slot {slot} is not in materials.ron");
                found += 1;
            }
            assert!(found > 0, "{key}: no `{key}_<Slot>` meshes");
        }
    }

    #[test]
    fn mip_chain_reaches_one_pixel_and_keeps_normals_unit_length() {
        let size = 8u32;
        let level0: Vec<u8> = (0..size * size).flat_map(|i| if i % 2 == 0 { [255, 128, 128, 255] } else { [128, 128, 255, 255] }).collect();
        let levels = mip_chain(level0, size, 4, Kind::Normal);
        assert_eq!(levels.len(), 4);
        assert_eq!(levels[3].len(), 4);
        let v = Vec3::new(levels[3][0] as f32 / 127.5 - 1.0, levels[3][1] as f32 / 127.5 - 1.0, levels[3][2] as f32 / 127.5 - 1.0);
        assert!((v.length() - 1.0).abs() < 0.02, "{v:?}");
    }
}
