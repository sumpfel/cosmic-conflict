//! Client networking: connecting (non-blocking), the message pump, and the embedded server
//! used by "Host Game".

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bevy::prelude::*;
use cc_core::{ContentDb, PlayerId};
use cc_protocol::transport::{Connection, Incoming};
use cc_protocol::*;
use crossbeam_channel::{Receiver, TryRecvError};

use crate::settings::Settings;
use crate::{AppState, Content, MenuScreen};

pub type ClientConn = Connection<ClientMsg, ServerMsg>;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum NetStatus {
    #[default]
    Idle,
    Connecting(String),
    Connected,
}

#[derive(Resource, Default)]
pub struct Net {
    conn: Option<ClientConn>,
    pending: Option<Receiver<Result<ClientConn, String>>>,
    pub status: NetStatus,
    pub server_name: String,
    pub me: Option<PlayerId>,
    pub ping_ms: f32,
    ping_timer: f32,
}

impl Net {
    pub fn connect(&mut self, addr: String) {
        self.disconnect();
        let (tx, rx) = crossbeam_channel::bounded(1);
        let a = addr.clone();
        std::thread::spawn(move || {
            let r = ClientConn::connect(&a, Duration::from_secs(5)).map_err(|e| format!("Could not connect to {a}: {e}"));
            let _ = tx.send(r);
        });
        self.pending = Some(rx);
        self.status = NetStatus::Connecting(addr);
    }

    pub fn send(&self, msg: ClientMsg) {
        if let Some(c) = &self.conn {
            c.send(&msg);
        }
    }

    pub fn command(&self, cmd: GameCommand) {
        self.send(ClientMsg::Command(cmd));
    }

    pub fn disconnect(&mut self) {
        if let Some(c) = self.conn.take() {
            c.send(&ClientMsg::Leave);
            c.close();
        }
        self.pending = None;
        self.me = None;
        self.status = NetStatus::Idle;
    }
}

/// The embedded server when this client hosts.
#[derive(Resource, Default)]
pub struct HostedServer(pub Option<cc_server::ServerHandle>);

impl HostedServer {
    pub fn start(&mut self, port: u16, content: Arc<ContentDb>) -> Result<(), String> {
        self.stop();
        let cfg = cc_server::ServerConfig { bind: format!("0.0.0.0:{port}"), name: "Hosted game".into() };
        let h = cc_server::start(cfg, content).map_err(|e| format!("Could not host on port {port}: {e}"))?;
        self.0 = Some(h);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut h) = self.0.take() {
            h.stop();
        }
    }
}

#[derive(Resource, Default)]
pub struct LobbyRes {
    pub state: Option<LobbyState>,
}

#[derive(Resource, Default)]
pub struct ChatLog {
    pub lines: Vec<(Option<PlayerId>, String)>,
    pub input: String,
}

/// The match being loaded / played.
#[derive(Resource, Default)]
pub struct MatchRes {
    pub start: Option<GameStart>,
    pub running: bool,
    pub game_over: Option<Option<PlayerId>>,
}

/// Snapshots waiting to be applied to the client world.
#[derive(Resource, Default)]
pub struct IncomingSnapshots(pub VecDeque<Snapshot>);

/// Messages for this player (shown in the HUD for a few seconds).
#[derive(Resource, Default)]
pub struct Notices(pub Vec<(String, f32)>);

impl Notices {
    pub fn push(&mut self, text: String) {
        self.0.retain(|(t, _)| *t != text);
        self.0.push((text, 6.0));
        if self.0.len() > 6 {
            self.0.remove(0);
        }
    }
}

/// A message on the menu (errors, kicks).
#[derive(Resource, Default)]
pub struct MenuMessage(pub Option<String>);

#[allow(clippy::too_many_arguments)]
pub fn pump_network(
    mut net: ResMut<Net>,
    content: Res<Content>,
    settings: Res<Settings>,
    mut lobby: ResMut<LobbyRes>,
    mut chat: ResMut<ChatLog>,
    mut mres: ResMut<MatchRes>,
    mut snaps: ResMut<IncomingSnapshots>,
    mut notices: ResMut<Notices>,
    mut menu_msg: ResMut<MenuMessage>,
    mut screen: ResMut<MenuScreen>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    if let Some(rx) = &net.pending {
        match rx.try_recv() {
            Ok(Ok(conn)) => {
                conn.send(&ClientMsg::Hello {
                    protocol: PROTOCOL_VERSION,
                    content_hash: content.0.hash,
                    name: settings.name.clone(),
                });
                net.conn = Some(conn);
                net.pending = None;
                net.status = NetStatus::Connected;
            }
            Ok(Err(e)) => {
                net.pending = None;
                net.status = NetStatus::Idle;
                menu_msg.0 = Some(e);
                *screen = MenuScreen::Main;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                net.pending = None;
                net.status = NetStatus::Idle;
            }
        }
    }
    net.ping_timer -= time.delta_secs();
    let ping_due = net.ping_timer <= 0.0;
    if ping_due {
        net.ping_timer = 2.0;
    }
    let Some(conn) = net.conn.as_ref() else { return };
    if ping_due {
        conn.send(&ClientMsg::Ping((time.elapsed_secs_f64() * 1000.0) as u64));
    }
    let mut msgs = Vec::new();
    let mut closed: Option<String> = None;
    for _ in 0..2000 {
        match conn.try_recv() {
            Some(Incoming::Msg(m)) => msgs.push(m),
            Some(Incoming::Closed(r)) => {
                closed = Some(r);
                break;
            }
            None => break,
        }
    }
    for m in msgs {
        match m {
            ServerMsg::Welcome { you, server_name } => {
                net.me = Some(you);
                net.server_name = server_name;
                net.send(ClientMsg::Lobby(LobbyCmd::SetColor(settings.color)));
                *screen = MenuScreen::Lobby;
                chat.lines.clear();
                menu_msg.0 = None;
            }
            ServerMsg::Rejected(reason) | ServerMsg::Kicked(reason) => {
                menu_msg.0 = Some(reason);
                closed = Some(String::new());
            }
            ServerMsg::Lobby(s) => lobby.state = Some(s),
            ServerMsg::Chat { from, text } => {
                chat.lines.push((from, text));
                if chat.lines.len() > 200 {
                    chat.lines.remove(0);
                }
            }
            ServerMsg::StartGame(gs) => {
                mres.start = Some(gs);
                mres.running = false;
                mres.game_over = None;
                snaps.0.clear();
                notices.0.clear();
                next.set(AppState::Loading);
            }
            ServerMsg::GameRunning => mres.running = true,
            ServerMsg::Snapshot(s) => snaps.0.push_back(*s),
            ServerMsg::Notice(text) => notices.push(text),
            ServerMsg::GameOver { winner } => mres.game_over = Some(winner),
            ServerMsg::Pong(t) => {
                let now = (time.elapsed_secs_f64() * 1000.0) as u64;
                net.ping_ms = now.saturating_sub(t) as f32;
            }
        }
    }
    if let Some(reason) = closed {
        if !reason.is_empty() && menu_msg.0.is_none() {
            menu_msg.0 = Some(format!("Disconnected: {reason}"));
        }
        net.conn = None;
        net.me = None;
        net.status = NetStatus::Idle;
        lobby.state = None;
        mres.start = None;
        *screen = MenuScreen::Main;
        if *state.get() != AppState::Menu {
            next.set(AppState::Menu);
        }
    }
}

/// Development helper: `CC_AUTOSTART=1` hosts a local game on port 47999, joins it and starts a
/// match immediately (used for screenshots and smoke tests). `CC_SEED`, `CC_SIZE=small|large`.
pub fn dev_autostart(
    mut stage: Local<u8>,
    mut net: ResMut<Net>,
    mut hosted: ResMut<HostedServer>,
    lobby: Res<LobbyRes>,
    content: Res<Content>,
) {
    if std::env::var("CC_AUTOSTART").is_err() {
        return;
    }
    match *stage {
        0 => {
            if hosted.start(47999, content.0.clone()).is_ok() {
                net.connect("127.0.0.1:47999".into());
            }
            *stage = 1;
        }
        1 => {
            if let (Some(me), Some(state)) = (net.me, lobby.state.as_ref())
                && state.host == me
            {
                let seed = std::env::var("CC_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(77);
                let size = match std::env::var("CC_SIZE").as_deref() {
                    Ok("small") => cc_core::settings::SystemSize::Small,
                    Ok("large") => cc_core::settings::SystemSize::Large,
                    _ => cc_core::settings::SystemSize::Medium,
                };
                let start = match std::env::var("CC_START").as_deref() {
                    Ok("high") => cc_core::settings::StartResources::High,
                    _ => cc_core::settings::StartResources::Normal,
                };
                let s = cc_core::settings::MatchSettings { seed, size, start, ..state.settings.clone() };
                net.send(ClientMsg::Lobby(LobbyCmd::Settings(s)));
                *stage = 2;
            }
        }
        2 if std::env::var("CC_AUTOSTART").as_deref() != Ok("lobby") => {
            net.send(ClientMsg::Lobby(LobbyCmd::Start));
            *stage = 3;
        }
        _ => {}
    }
}
