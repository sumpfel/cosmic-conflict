//! Sound effects. Other systems request sounds through the `Sound` resource
//! (`sound.ui(Sfx::Order)`); they are played once per frame, never twice the same.
//! `CC_MUTE=1` silences everything (test runs).

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Select,
    Order,
    Attack,
    Place,
    Error,
    BuildingDone,
    UnitReady,
    Explosion,
    Click,
}

impl Sfx {
    fn file(self) -> &'static str {
        match self {
            Sfx::Select => "audio/gem_1.ogg",
            Sfx::Order => "audio/order.ogg",
            Sfx::Attack => "audio/order_attack.ogg",
            Sfx::Place => "audio/place.ogg",
            Sfx::Error => "audio/metal_1.ogg",
            Sfx::BuildingDone => "audio/building_done.ogg",
            Sfx::UnitReady => "audio/unit_ready.ogg",
            Sfx::Explosion => "audio/destroy_1.ogg",
            Sfx::Click => "audio/ui_click.ogg",
        }
    }

    fn gain(self) -> f32 {
        match self {
            Sfx::Select => 0.25,
            Sfx::Click => 0.5,
            Sfx::Explosion => 0.5,
            _ => 0.7,
        }
    }
}

#[derive(Resource, Default)]
pub struct Sound {
    queue: Vec<Sfx>,
}

impl Sound {
    pub fn ui(&mut self, s: Sfx) {
        if !self.queue.contains(&s) {
            self.queue.push(s);
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sound>().add_systems(PostUpdate, play_sounds);
    }
}

fn play_sounds(mut commands: Commands, mut sound: ResMut<Sound>, assets: Res<AssetServer>, settings: Res<Settings>) {
    let muted = std::env::var("CC_MUTE").is_ok();
    for s in sound.queue.drain(..) {
        if muted || settings.volume <= 0.0 {
            continue;
        }
        commands.spawn((
            AudioPlayer::new(assets.load(s.file())),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(s.gain() * settings.volume)),
        ));
    }
}
