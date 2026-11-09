//! # cc_protocol — client/server messages
//!
//! Server-authoritative: clients send intents ([`GameCommand`]), the server validates them,
//! runs the simulation at [`TICK_RATE`] Hz and sends a [`Snapshot`] every [`SNAPSHOT_EVERY`]
//! ticks. Frames are `[u32 len][postcard]` over TCP (see [`transport`]).

pub mod transport;

use cc_core::PlayerId;
use cc_core::content::{BuildingKind, UnitKind};
use cc_core::settings::MatchSettings;
use serde::{Deserialize, Serialize};

pub use cc_core::system::NO_PLANET;

/// Bump on every incompatible message change.
pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 47900;
pub const TICK_RATE: u32 = 20;
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;
/// A snapshot is sent every this many ticks (10 Hz).
pub const SNAPSHOT_EVERY: u64 = 2;

pub type EntityId = u32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub id: PlayerId,
    pub name: String,
    pub color: u8,
    pub ready: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyState {
    pub players: Vec<LobbyPlayer>,
    pub host: PlayerId,
    pub settings: MatchSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LobbyCmd {
    SetColor(u8),
    SetReady(bool),
    /// Host only.
    Settings(MatchSettings),
    /// Host only.
    Start,
    /// Host only.
    Kick(PlayerId),
    Chat(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchPlayer {
    pub id: PlayerId,
    pub name: String,
    pub color: u8,
    /// Index into the generated star system's home planets.
    pub slot: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameStart {
    pub settings: MatchSettings,
    pub players: Vec<MatchPlayer>,
    pub you: PlayerId,
    /// Hash of the generated star system; the client compares after generating it.
    pub map_hash: u64,
}

/// Where a move order goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// A point on a planet's surface (unit direction from its centre).
    Ground { planet: u16, dir: [f32; 3] },
    /// A point in space.
    Space { pos: [f32; 3] },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameCommand {
    /// Ground units walk on their planet; ships fly (to low orbit when the target is a planet).
    Move { units: Vec<EntityId>, to: Target },
    Attack { units: Vec<EntityId>, target: EntityId },
    Stop { units: Vec<EntityId> },
    /// Builders start a building (pays the cost). A colonizer flies there and founds it.
    Build { builders: Vec<EntityId>, kind: BuildingKind, planet: u16, dir: [f32; 3] },
    /// Builders help finishing an own construction site.
    Assist { builders: Vec<EntityId>, building: EntityId },
    Train { building: EntityId, kind: UnitKind },
    /// Removes the last queued unit (refunded).
    CancelTrain { building: EntityId },
    Demolish { building: EntityId },
    Surrender,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { protocol: u32, content_hash: u64, name: String },
    Lobby(LobbyCmd),
    /// The client generated the star system and is ready to play.
    MapReady { map_hash: u64 },
    Command(GameCommand),
    Ping(u64),
    Leave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Idle,
    Moving,
    Attacking,
    Building,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitSnap {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: UnitKind,
    /// The planet a ground unit stands on, `NO_PLANET` for ships.
    pub planet: u16,
    /// Ground: unit direction from the planet's centre. Space: world position.
    pub pos: [f32; 3],
    pub hp: f32,
    pub activity: Activity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingSnap {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: BuildingKind,
    pub planet: u16,
    pub dir: [f32; 3],
    pub hp: f32,
    /// Construction progress 0..1 (1 = finished).
    pub progress: f32,
    pub queue: Vec<UnitKind>,
    /// Progress of the first unit in the queue 0..1.
    pub train_progress: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnap {
    pub id: PlayerId,
    pub stock: [f32; 3],
    /// Resources per second right now.
    pub income: [f32; 3],
    pub pop: u32,
    pub pop_cap: u32,
    pub defeated: bool,
}

/// A weapon fired (for visual effects).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shot {
    pub from: EntityId,
    pub to: EntityId,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub players: Vec<PlayerSnap>,
    pub units: Vec<UnitSnap>,
    pub buildings: Vec<BuildingSnap>,
    /// Deposits whose amount changed since the previous snapshot: (id, amount left).
    pub deposits: Vec<(u32, f32)>,
    pub shots: Vec<Shot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { you: PlayerId, server_name: String },
    Rejected(String),
    Kicked(String),
    Lobby(LobbyState),
    Chat { from: Option<PlayerId>, text: String },
    StartGame(GameStart),
    /// Every client has loaded; the simulation runs.
    GameRunning,
    Snapshot(Box<Snapshot>),
    /// A message for this player only (e.g. "not enough metal").
    Notice(String),
    GameOver { winner: Option<PlayerId> },
    Pong(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let m = ClientMsg::Command(GameCommand::Build { builders: vec![1, 2], kind: 3, planet: 0, dir: [0.0, 1.0, 0.0] });
        let bytes = postcard::to_allocvec(&m).unwrap();
        let back: ClientMsg = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(back, ClientMsg::Command(GameCommand::Build { kind: 3, .. })));
    }
}
