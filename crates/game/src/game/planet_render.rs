//! Realistic planets: the surface shader (`assets/shaders/planet.wesl`), the atmosphere and
//! cloud shell (`atmosphere.wesl`), the sun's surface (`sun.wesl`), the sun's light rig with a
//! shadow-casting directional light aimed at the planet in view, and the graphics settings
//! (presets for weaker PCs).
//!
//! Lighting: the planet shaders light themselves from the true sun direction of every planet.
//! Units, buildings and ships use Bevy's lights: a point light in the sun (correct everywhere,
//! no shadows) cross-faded with a directional light from the sun towards the focused planet
//! (with cascaded shadow maps) when the camera is near that planet. Both give the same
//! illuminance there, so the cross-fade is invisible apart from the shadows appearing.

use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster, NotShadowReceiver, ShadowFilteringMethod};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use cc_core::system::{Biome, Planet};

use super::camera::OrbitCam;
use super::planet_mesh::atmosphere;
use super::scene::{MainCamera, MenuBackdrop, SunLight};
use crate::settings::{AntiAliasing, Graphics, Level, Settings};

/// Illuminance (lux) the sun gives the planet in focus; other planets fall off physically.
pub const FOCUS_LUX: f32 = 2400.0;
/// Colour of the sunlight (sRGB).
pub const SUN_COLOR: Color = Color::srgb(1.0, 0.93, 0.85);

pub struct PlanetRenderPlugin;

impl Plugin for PlanetRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<PlanetMaterial>::default(),
            MaterialPlugin::<AtmosphereMaterial>::default(),
            MaterialPlugin::<SunMaterial>::default(),
        ))
        .init_resource::<SunRig>()
        .insert_resource(DirectionalLightShadowMap { size: 2048 })
        .add_systems(Update, (apply_graphics, update_sun_rig, update_planet_materials).chain());
    }
}

/// Per-planet shader parameters (mirrors `PlanetParams` in `planet_common.wesl`).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct PlanetParams {
    pub center: Vec4,
    pub orient: Vec4,
    pub shape: Vec4,
    pub sun: Vec4,
    pub sun_color: Vec4,
    pub atmo: Vec4,
    pub atmo2: Vec4,
    pub rock: Vec4,
    pub cloud: Vec4,
    pub cloud2: Vec4,
    pub quality: Vec4,
}

impl PlanetParams {
    fn fields(&self) -> [Vec4; 11] {
        [
            self.center,
            self.orient,
            self.shape,
            self.sun,
            self.sun_color,
            self.atmo,
            self.atmo2,
            self.rock,
            self.cloud,
            self.cloud2,
            self.quality,
        ]
    }

    /// True when worth re-uploading (avoids touching the material assets every frame).
    fn differs(&self, other: &PlanetParams) -> bool {
        self.fields().iter().zip(other.fields()).any(|(a, b)| {
            let d = (*a - b).abs().max_element();
            d > 1e-4 * (1.0 + a.abs().max_element())
        })
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct PlanetMaterial {
    #[uniform(0)]
    pub params: PlanetParams,
}

impl Material for PlanetMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet.wesl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct AtmosphereMaterial {
    #[uniform(0)]
    pub params: PlanetParams,
}

impl Material for AtmosphereMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/atmosphere.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Front faces from outside, back faces from inside (the shader picks one).
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SunMaterial {
    /// rgb: emissive colour, w: surface detail on/off.
    #[uniform(0)]
    pub color: Vec4,
    /// x: 0 = surface, 1 = corona shell; y: sun radius; z: corona strength.
    #[uniform(0)]
    pub shape: Vec4,
}

impl SunMaterial {
    pub fn surface(color: Vec3) -> Self {
        SunMaterial { color: color.extend(1.0), shape: Vec4::ZERO }
    }

    pub fn corona(color: Vec3, sun_radius: f32, strength: f32) -> Self {
        SunMaterial { color: color.extend(0.0), shape: Vec4::new(1.0, sun_radius, strength, 0.0) }
    }
}

impl Material for SunMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sun.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.shape.x > 0.5 { AlphaMode::Add } else { AlphaMode::Opaque }
    }

    fn enable_shadows() -> bool {
        false
    }

    /// The corona shell must stay out of the depth prepass, or it would hide everything behind
    /// it (the prepass decides which opaque pixels survive). The surface does not need it either.
    fn enable_prepass() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The corona shell shades its front faces from outside, back faces from inside.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The directional light that carries the sun's shadows.
#[derive(Component)]
pub struct SunShadowLight;

/// On every planet globe: its static look and its materials.
#[derive(Component)]
pub struct PlanetLook {
    pub id: u16,
    pub radius: f32,
    base: PlanetParams,
    material: Handle<PlanetMaterial>,
    /// The atmosphere / cloud shell (child entity) and its material.
    shell: Option<(Entity, Handle<AtmosphereMaterial>)>,
    written: Option<(PlanetParams, PlanetParams)>,
}

/// The eased state of the sun's lights.
#[derive(Resource, Default)]
pub struct SunRig {
    /// Total luminous power of the sun (lm), scaled so the focused planet gets `FOCUS_LUX`.
    lumens: f32,
    /// 0 = only the point light, 1 = only the directional (shadow) light.
    shadow_mix: f32,
    /// The planet the shadows are for.
    target: Option<Entity>,
}

struct BiomeLook {
    density: f32,
    haze: f32,
    rock: [f32; 3],
    cloud: [f32; 3],
    coverage: f32,
}

fn biome_look(b: Biome) -> BiomeLook {
    let l = |density, haze, rock, cloud, coverage| BiomeLook { density, haze, rock, cloud, coverage };
    match b {
        Biome::Terran => l(1.0, 0.45, [0.30, 0.28, 0.25], [0.96, 0.96, 0.98], 0.52),
        Biome::Jungle => l(1.15, 0.8, [0.26, 0.25, 0.20], [0.93, 0.96, 0.94], 0.62),
        Biome::Desert => l(0.55, 1.6, [0.50, 0.33, 0.21], [0.85, 0.70, 0.52], 0.16),
        Biome::Ice => l(0.7, 0.35, [0.40, 0.43, 0.48], [0.92, 0.95, 1.0], 0.38),
        Biome::Volcanic => l(0.85, 1.8, [0.11, 0.09, 0.085], [0.30, 0.27, 0.25], 0.34),
        Biome::Barren => l(0.0, 0.0, [0.36, 0.35, 0.33], [0.0; 3], 0.0),
        Biome::Toxic => l(1.25, 1.2, [0.28, 0.23, 0.27], [0.74, 0.80, 0.42], 0.46),
    }
}

/// The parts of the parameters that never change for a planet.
pub fn base_params(p: &Planet) -> PlanetParams {
    let look = biome_look(p.biome);
    let tint = atmosphere(p.biome).unwrap_or([0.0; 3]);
    let density = if atmosphere(p.biome).is_some() { look.density } else { 0.0 };
    let sea_h = p.biome.sea_level().map(|s| s * p.relief).unwrap_or(-1e6);
    let sea_kind = match p.biome {
        Biome::Terran | Biome::Jungle => 1.0,
        Biome::Ice => 2.0,
        Biome::Volcanic => 3.0,
        Biome::Toxic => 4.0,
        Biome::Desert | Biome::Barren => 0.0,
    };
    let lava_cracks = if p.biome == Biome::Volcanic { 1.0 } else { 0.0 };
    let ground_top = p.radius + p.relief;
    let air_top = ground_top + p.radius * 0.12;
    let seed = (p.seed % 997) as f32 * 0.731;
    PlanetParams {
        center: p.center.extend(p.radius),
        orient: Vec4::new(0.0, 0.0, 0.0, 1.0),
        shape: Vec4::new(p.relief, sea_h, sea_kind, lava_cracks),
        sun: Vec4::new(0.0, 1.0, 0.0, 0.0),
        sun_color: SUN_COLOR.to_linear().to_vec3().extend(0.0),
        atmo: Vec3::from(tint).extend(density),
        atmo2: Vec4::new(air_top, (air_top - p.radius) * 0.2, look.haze, 1.7),
        rock: Vec3::from(look.rock).extend(seed),
        cloud: Vec3::from(look.cloud).extend(look.coverage),
        cloud2: Vec4::new(ground_top + p.radius * 0.035, 1.0, 0.55, 0.006),
        quality: Vec4::new(3.0, 12.0, 4.0, 5.0),
    }
}

/// Material handles needed to spawn planets.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlanetAssets<'w> {
    pub planets: ResMut<'w, Assets<PlanetMaterial>>,
    pub shells: ResMut<'w, Assets<AtmosphereMaterial>>,
}

/// Spawns a planet globe with its surface material and (if it has air or clouds) the
/// atmosphere shell as a child. Returns the globe's entity.
pub fn spawn_planet_visual(
    commands: &mut Commands,
    assets: &mut PlanetAssets,
    sphere: Handle<Mesh>,
    p: &Planet,
    mesh: Handle<Mesh>,
    marker: impl Component,
) -> Entity {
    let base = base_params(p);
    let material = assets.planets.add(PlanetMaterial { params: base });
    let e = commands.spawn((marker, Mesh3d(mesh), MeshMaterial3d(material.clone()), Transform::from_translation(p.center))).id();
    let shell = (base.atmo.w > 0.0 || base.cloud.w > 0.0).then(|| {
        let m = assets.shells.add(AtmosphereMaterial { params: base });
        let child = commands
            .spawn((
                Mesh3d(sphere),
                MeshMaterial3d(m.clone()),
                Transform::from_scale(Vec3::splat(base.atmo2.x * 1.01)),
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        commands.entity(e).add_child(child);
        (child, m)
    });
    commands.entity(e).insert(PlanetLook { id: p.id, radius: p.radius, base, material, shell, written: None });
    e
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn shadow_setup(level: Level) -> Option<(usize, usize, ShadowFilteringMethod)> {
    match level {
        Level::Off => None,
        Level::Low => Some((1024, 2, ShadowFilteringMethod::Hardware2x2)),
        Level::Medium => Some((2048, 3, ShadowFilteringMethod::Gaussian)),
        Level::High => Some((2048, 4, ShadowFilteringMethod::Gaussian)),
        Level::Ultra => Some((4096, 4, ShadowFilteringMethod::Gaussian)),
    }
}

/// Applies the graphics settings to the camera and the shadow map when they change.
pub fn apply_graphics(
    mut commands: Commands,
    settings: Res<Settings>,
    mut last: Local<Option<Graphics>>,
    camera: Query<Entity, With<MainCamera>>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    let g = settings.graphics;
    let Ok(cam) = camera.single() else { return };
    if *last == Some(g) {
        return;
    }
    *last = Some(g);
    let mut c = commands.entity(cam);
    match g.antialiasing {
        AntiAliasing::Off => {
            c.insert(Msaa::Off).remove::<Fxaa>();
        }
        AntiAliasing::Fxaa => {
            c.insert((Msaa::Off, Fxaa::default()));
        }
        AntiAliasing::Msaa4 => {
            c.insert(Msaa::Sample4).remove::<Fxaa>();
        }
    }
    // The atmosphere shader stops its rays at the scene depth (and the depth prepass spares the
    // planet shader the pixels hidden behind buildings).
    if g.atmosphere != Level::Off || g.clouds != Level::Off {
        c.insert(DepthPrepass);
    } else {
        c.remove::<DepthPrepass>();
    }
    if g.bloom {
        c.insert(Bloom::NATURAL);
    } else {
        c.remove::<Bloom>();
    }
    if let Some((size, _, filter)) = shadow_setup(g.shadows) {
        c.insert(filter);
        if shadow_map.size != size {
            shadow_map.size = size;
        }
    }
    info!("graphics: {g:?}");
}

/// The sun's lights follow the camera's focus: the point light everywhere, the directional
/// shadow light near the focused planet, with cascades fitted to the visible part of it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_sun_rig(
    time: Res<Time>,
    settings: Res<Settings>,
    cam: Option<Res<OrbitCam>>,
    mut rig: ResMut<SunRig>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    planets: Query<(Entity, &GlobalTransform, &PlanetLook, Has<MenuBackdrop>)>,
    mut point: Query<&mut PointLight, With<SunLight>>,
    mut dir: Query<(&mut DirectionalLight, &mut Transform, &mut CascadeShadowConfig), With<SunShadowLight>>,
) {
    let Ok(cam_t) = camera.single() else { return };
    let cam_pos = cam_t.translation();
    let dt = time.delta_secs().min(0.1);
    // The planet in view: the menu globe, or the camera's focus.
    let focus = cam.as_ref().and_then(|c| c.focus);
    let target = planets
        .iter()
        .find(|(.., backdrop)| *backdrop)
        .or_else(|| focus.and_then(|f| planets.iter().find(|(_, _, look, _)| look.id == f)))
        .map(|(e, t, look, _)| (e, t.translation(), look.radius));
    let dist = match target {
        Some((_, c, _)) => c.length(),
        None if !planets.is_empty() => planets.iter().map(|(_, t, ..)| t.translation().length()).sum::<f32>() / planets.iter().count() as f32,
        None => 1000.0,
    };
    let lumens = FOCUS_LUX * 4.0 * std::f32::consts::PI * dist * dist;
    // Ease so focus changes do not flash.
    let ease = 1.0 - (-dt * 5.0).exp();
    rig.lumens = if rig.lumens <= 0.0 { lumens } else { rig.lumens + (lumens - rig.lumens) * ease };

    let shadows = shadow_setup(settings.graphics.shadows);
    let mix_target = match (target, shadows) {
        (Some((_, c, r)), Some(_)) => 1.0 - smoothstep(6.0 * r, 14.0 * r, cam_pos.distance(c)),
        _ => 0.0,
    };
    if rig.target != target.map(|t| t.0) {
        // New planet: the shadow light jumps there, start from the point light.
        rig.target = target.map(|t| t.0);
        rig.shadow_mix = 0.0;
    }
    rig.shadow_mix += (mix_target - rig.shadow_mix) * (1.0 - (-dt * 3.0).exp());
    if rig.shadow_mix < 0.002 && mix_target == 0.0 {
        rig.shadow_mix = 0.0;
    }
    let k = rig.shadow_mix;

    for mut l in &mut point {
        l.intensity = rig.lumens * (1.0 - k);
    }
    let Ok((mut light, mut t, mut cascades)) = dir.single_mut() else { return };
    let Some((_, center, radius)) = target else {
        light.illuminance = 0.0;
        light.shadow_maps_enabled = false;
        return;
    };
    let to_planet = center.normalize_or(Vec3::X);
    let up = if to_planet.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    let wanted = Transform::default().looking_to(to_planet, up);
    if t.rotation.angle_between(wanted.rotation) > 1e-4 {
        *t = wanted;
    }
    light.illuminance = rig.lumens / (4.0 * std::f32::consts::PI * dist * dist) * k;
    let enabled = k > 0.01 && shadows.is_some();
    if light.shadow_maps_enabled != enabled {
        light.shadow_maps_enabled = enabled;
    }
    if let Some((_, n, _)) = shadows.filter(|_| enabled) {
        // Fit the cascades to the visible part of the planet: from the camera to a bit past
        // the horizon, the first one about as deep as the camera is high.
        let d = cam_pos.distance(center).max(radius * 1.01);
        let horizon = (d * d - radius * radius).max(0.0).sqrt();
        let max_d = (horizon + radius * 0.35).max(20.0);
        let first = ((d - radius) * 1.3).clamp(4.0, max_d * 0.6);
        let current_max = cascades.bounds.last().copied().unwrap_or(0.0);
        if cascades.bounds.len() != n || (current_max - max_d).abs() > max_d * 0.03 {
            *cascades = CascadeShadowConfigBuilder {
                num_cascades: n,
                minimum_distance: 0.1,
                maximum_distance: max_d,
                first_cascade_far_bound: first,
                overlap_proportion: 0.25,
            }
            .build();
        }
    }
}

/// Writes the per-frame parts (sun, shadow mix, cloud fade, orientation) and the quality
/// settings into every planet's materials.
#[allow(clippy::type_complexity)]
pub fn update_planet_materials(
    settings: Res<Settings>,
    rig: Res<SunRig>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut planets: Query<(Entity, &GlobalTransform, &mut PlanetLook)>,
    mut shells: Query<&mut Visibility>,
    mut planet_mats: ResMut<Assets<PlanetMaterial>>,
    mut shell_mats: ResMut<Assets<AtmosphereMaterial>>,
) {
    let Ok(cam_t) = camera.single() else { return };
    let cam_pos = cam_t.translation();
    let g = settings.graphics;
    let surface = g.surface.index().max(1) as f32;
    let (steps, light_steps) = match g.atmosphere {
        Level::Off => (1.0, 0.0),
        Level::Low => (5.0, 0.0),
        Level::Medium => (8.0, 2.0),
        Level::High => (12.0, 3.0),
        Level::Ultra => (16.0, 4.0),
    };
    let cloud_octaves = match g.clouds {
        Level::Off => 0.0,
        Level::Low => 3.0,
        Level::Medium => 4.0,
        Level::High => 5.0,
        Level::Ultra => 6.0,
    };
    for (e, t, mut look) in &mut planets {
        let center = t.translation();
        let radius = look.radius;
        let mut p = look.base;
        p.center = center.extend(radius);
        let q = t.rotation();
        p.orient = Vec4::new(q.x, q.y, q.z, q.w);
        let d_sun = center.length().max(1.0);
        p.sun = (-center / d_sun).extend(rig.lumens / (4.0 * std::f32::consts::PI * d_sun * d_sun));
        p.sun_color.w = if rig.target == Some(e) { rig.shadow_mix } else { 0.0 };
        // Clouds hide the ground: thin them out when the camera comes down to it.
        let altitude = cam_pos.distance(center) - radius;
        p.cloud2.y = 0.15 + 0.85 * smoothstep(0.55 * radius, 2.2 * radius, altitude);
        if g.clouds == Level::Off {
            p.cloud.w = 0.0;
        }
        if g.clouds < Level::Medium {
            p.cloud2.z = 0.0;
        }
        p.quality = Vec4::new(surface, steps, light_steps, cloud_octaves);
        let mut shell = p;
        if g.atmosphere == Level::Off {
            shell.atmo.w = 0.0;
        }
        let changed = look.written.is_none_or(|(a, b)| a.differs(&p) || b.differs(&shell));
        if !changed {
            continue;
        }
        look.written = Some((p, shell));
        if let Some(mut m) = planet_mats.get_mut(&look.material) {
            m.params = p;
        }
        if let Some((child, handle)) = look.shell.clone() {
            let show = shell.atmo.w > 0.0 || shell.cloud.w > 0.0;
            if let Ok(mut v) = shells.get_mut(child) {
                let want = if show { Visibility::Inherited } else { Visibility::Hidden };
                if *v != want {
                    *v = want;
                }
            }
            if let Some(mut m) = shell_mats.get_mut(&handle) {
                m.params = shell;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet(biome: Biome) -> Planet {
        Planet {
            id: 3,
            name: "T".into(),
            center: Vec3::new(500.0, 0.0, 0.0),
            radius: 60.0,
            biome,
            seed: 77,
            home: None,
            start_dir: Vec3::Y,
            relief: 2.4,
        }
    }

    #[test]
    fn layers_are_ordered_ground_clouds_air_orbit() {
        for b in Biome::ALL {
            let p = planet(b);
            let pp = base_params(&p);
            let ground_top = p.radius + p.relief;
            if pp.cloud.w > 0.0 {
                assert!(pp.atmo.w > 0.0, "{b:?}: clouds need air");
                assert!(pp.cloud2.x > ground_top && pp.cloud2.x < pp.atmo2.x, "{b:?}: clouds between ground and the top of the air");
            }
            if pp.atmo.w > 0.0 {
                // Ships hover above the air, not inside the scattering shell.
                assert!(pp.atmo2.x < p.orbit_point(Vec3::Y).distance(p.center), "{b:?}");
                assert!(pp.atmo2.y > 0.0);
            }
        }
        assert_eq!(base_params(&planet(Biome::Barren)).atmo.w, 0.0);
    }

    #[test]
    fn small_changes_are_not_reuploaded() {
        let a = base_params(&planet(Biome::Terran));
        let mut b = a;
        b.sun.w += a.sun.w * 1e-6;
        assert!(!a.differs(&b));
        b.cloud2.y = 0.5;
        assert!(a.differs(&b));
    }
}
