//! # cc_server — lobby and match server
//!
//! Runs either as the dedicated `cc-server` binary or embedded in the game client
//! ("Host Game"). One server hosts one lobby/match at a time.
//!
//! Lifecycle: `Lobby` → host presses start → `Loading` (every client generates the star system
//! from the seed and reports its hash) → `Running` (20 Hz simulation, 10 Hz snapshots) →
//! `GameOver` → back to `Lobby`.

use std::collections::HashSet;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cc_core::rng::Rng;
use cc_core::settings::MatchSettings;
use cc_core::{ContentDb, PLAYER_COLORS, PlayerId, system};
use cc_protocol::transport::{Connection, Incoming, Listener, encode};
use cc_protocol::*;
use cc_sim::{PlayerInit, Sim};

type ServerConn = Connection<ServerMsg, ClientMsg>;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Address to listen on, e.g. "0.0.0.0:47900".
    pub bind: String,
    pub name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig { bind: format!("0.0.0.0:{DEFAULT_PORT}"), name: "Cosmic Conflict Server".into() }
    }
}

/// Handle to a running server thread. Dropping it stops the server.
pub struct ServerHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub addr: SocketAddr,
}

impl ServerHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Blocks until the server stops (dedicated server).
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts the server on a background thread.
pub fn start(config: ServerConfig, content: Arc<ContentDb>) -> io::Result<ServerHandle> {
    let listener: Listener<ServerMsg, ClientMsg> = Listener::bind(&config.bind)?;
    let addr = listener.local_addr;
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let thread = thread::Builder::new().name("cc-server".into()).spawn(move || {
        let mut server = Server::new(config, content, listener);
        server.run(&stop2);
    })?;
    log::info!("Cosmic Conflict server listening on {addr}");
    Ok(ServerHandle { stop, thread: Some(thread), addr })
}

struct Client {
    conn: ServerConn,
    /// Assigned after a valid Hello.
    player: Option<LobbyPlayer>,
    joined_at: Instant,
    order: u64,
}

enum Phase {
    Lobby,
    Loading { sim: Box<Sim>, ready: HashSet<PlayerId>, map_hash: u64, since: Instant },
    Running { sim: Box<Sim> },
    GameOver { until: Instant },
}

struct Server {
    config: ServerConfig,
    content: Arc<ContentDb>,
    listener: Listener<ServerMsg, ClientMsg>,
    clients: Vec<Client>,
    settings: MatchSettings,
    phase: Phase,
    join_counter: u64,
}

fn sanitize_name(n: &str) -> String {
    let s: String = n.chars().filter(|c| !c.is_control()).take(24).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "Commander".into() } else { s }
}

impl Server {
    fn new(config: ServerConfig, content: Arc<ContentDb>, listener: Listener<ServerMsg, ClientMsg>) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(7);
        Server {
            config,
            content,
            listener,
            clients: Vec::new(),
            settings: MatchSettings { seed: seed % 1_000_000_000, ..Default::default() },
            phase: Phase::Lobby,
            join_counter: 0,
        }
    }

    fn host(&self) -> Option<PlayerId> {
        self.clients
            .iter()
            .filter(|c| c.player.is_some())
            .min_by_key(|c| c.order)
            .and_then(|c| c.player.as_ref().map(|p| p.id))
    }

    fn members(&self) -> impl Iterator<Item = &LobbyPlayer> {
        self.clients.iter().filter_map(|c| c.player.as_ref())
    }

    fn lobby_state(&self) -> LobbyState {
        let mut players: Vec<LobbyPlayer> = self.members().cloned().collect();
        players.sort_by_key(|p| p.id);
        LobbyState { players, host: self.host().unwrap_or(0), settings: self.settings.clone() }
    }

    fn broadcast(&self, msg: &ServerMsg) {
        let bytes = encode(msg);
        for c in self.clients.iter().filter(|c| c.player.is_some()) {
            c.conn.send_raw(bytes.clone());
        }
    }

    fn broadcast_lobby(&self) {
        self.broadcast(&ServerMsg::Lobby(self.lobby_state()));
    }

    fn system_chat(&self, text: String) {
        self.broadcast(&ServerMsg::Chat { from: None, text });
    }

    fn free_player_id(&self) -> Option<PlayerId> {
        (0..MatchSettings::MAX_PLAYERS).find(|id| !self.members().any(|p| p.id == *id))
    }

    fn free_color(&self, preferred: u8) -> u8 {
        let used: Vec<u8> = self.members().map(|p| p.color).collect();
        if !used.contains(&preferred) {
            return preferred;
        }
        (0..PLAYER_COLORS.len() as u8).find(|c| !used.contains(c)).unwrap_or(0)
    }

    fn run(&mut self, stop: &AtomicBool) {
        let tick = Duration::from_secs_f32(TICK_DT);
        let mut next = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            self.accept();
            self.poll_clients();
            self.advance();
            next += tick;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else if now - next > Duration::from_millis(500) {
                next = now;
            }
        }
        self.broadcast(&ServerMsg::Kicked("The server was shut down.".into()));
        thread::sleep(Duration::from_millis(100));
        log::info!("server stopped");
    }

    fn accept(&mut self) {
        while let Some(conn) = self.listener.try_accept() {
            log::info!("connection from {}", conn.peer);
            self.join_counter += 1;
            self.clients.push(Client { conn, player: None, joined_at: Instant::now(), order: self.join_counter });
        }
    }

    fn poll_clients(&mut self) {
        let mut i = 0;
        while i < self.clients.len() {
            let mut msgs = Vec::new();
            let mut closed = None;
            for _ in 0..256 {
                match self.clients[i].conn.try_recv() {
                    Some(Incoming::Msg(m)) => msgs.push(m),
                    Some(Incoming::Closed(r)) => {
                        closed = Some(r);
                        break;
                    }
                    None => break,
                }
            }
            for m in msgs {
                self.handle(i, m);
            }
            if self.clients[i].player.is_none() && self.clients[i].joined_at.elapsed() > Duration::from_secs(10) {
                closed = Some("no hello".into());
            }
            if self.clients[i].conn.pending() > 2000 {
                closed = Some("too slow".into());
            }
            if closed.is_some() || !self.clients[i].conn.is_alive() {
                let c = self.clients.remove(i);
                self.on_leave(c, closed.unwrap_or_else(|| "disconnected".into()));
            } else {
                i += 1;
            }
        }
    }

    fn on_leave(&mut self, c: Client, reason: String) {
        c.conn.close();
        let Some(p) = c.player else { return };
        log::info!("{} left ({reason})", p.name);
        if let Phase::Loading { sim, .. } | Phase::Running { sim } = &mut self.phase {
            sim.remove_player(p.id);
        }
        self.system_chat(format!("{} left the game.", p.name));
        self.broadcast_lobby();
    }

    fn handle(&mut self, ci: usize, msg: ClientMsg) {
        let pid = self.clients[ci].player.as_ref().map(|p| p.id);
        match msg {
            ClientMsg::Hello { protocol, content_hash, name } => {
                if pid.is_some() {
                    return;
                }
                let reject = |s: &Self, why: String| {
                    s.clients[ci].conn.send(&ServerMsg::Rejected(why));
                };
                if protocol != PROTOCOL_VERSION {
                    reject(self, format!("Version mismatch: server protocol {PROTOCOL_VERSION}, yours {protocol}."));
                    return;
                }
                if content_hash != self.content.hash {
                    reject(self, "Game data differs from the server (different game version).".into());
                    return;
                }
                if !matches!(self.phase, Phase::Lobby) {
                    reject(self, "A match is in progress. Try again later.".into());
                    return;
                }
                if self.members().count() >= self.settings.max_players as usize {
                    reject(self, "The lobby is full.".into());
                    return;
                }
                let Some(id) = self.free_player_id() else {
                    reject(self, "The lobby is full.".into());
                    return;
                };
                let name = sanitize_name(&name);
                let color = self.free_color(id);
                self.clients[ci].player = Some(LobbyPlayer { id, name: name.clone(), color, ready: false });
                self.clients[ci].conn.send(&ServerMsg::Welcome { you: id, server_name: self.config.name.clone() });
                self.broadcast_lobby();
                self.system_chat(format!("{name} joined."));
            }
            ClientMsg::Lobby(LobbyCmd::Chat(text)) => {
                let Some(id) = pid else { return };
                let text: String = text.chars().filter(|c| !c.is_control()).take(200).collect();
                if !text.trim().is_empty() {
                    self.broadcast(&ServerMsg::Chat { from: Some(id), text });
                }
            }
            ClientMsg::Lobby(cmd) => {
                let Some(id) = pid else { return };
                if matches!(self.phase, Phase::Lobby) {
                    self.lobby_cmd(ci, id, cmd);
                }
            }
            ClientMsg::MapReady { map_hash } => {
                let Some(id) = pid else { return };
                if let Phase::Loading { ready, map_hash: expected, .. } = &mut self.phase {
                    if map_hash != *expected {
                        self.clients[ci]
                            .conn
                            .send(&ServerMsg::Kicked("Your generated star system differs from the server's.".into()));
                        self.clients[ci].conn.close();
                        return;
                    }
                    ready.insert(id);
                }
            }
            ClientMsg::Command(cmd) => {
                let Some(id) = pid else { return };
                if let Phase::Running { sim } = &mut self.phase {
                    sim.command(id, cmd);
                }
            }
            ClientMsg::Ping(t) => {
                self.clients[ci].conn.send(&ServerMsg::Pong(t));
            }
            ClientMsg::Leave => {
                self.clients[ci].conn.close();
            }
        }
    }

    fn lobby_cmd(&mut self, ci: usize, id: PlayerId, cmd: LobbyCmd) {
        let is_host = self.host() == Some(id);
        match cmd {
            LobbyCmd::SetColor(c) => {
                if (c as usize) < PLAYER_COLORS.len() {
                    let taken = self.members().any(|p| p.color == c && p.id != id);
                    if !taken && let Some(p) = self.clients[ci].player.as_mut() {
                        p.color = c;
                    }
                }
            }
            LobbyCmd::SetReady(r) => {
                if let Some(p) = self.clients[ci].player.as_mut() {
                    p.ready = r;
                }
            }
            LobbyCmd::Settings(mut s) => {
                if !is_host {
                    return;
                }
                s.max_players = s.max_players.clamp(1, MatchSettings::MAX_PLAYERS);
                self.settings = s;
                for c in self.clients.iter_mut() {
                    if let Some(p) = c.player.as_mut()
                        && p.id != id
                    {
                        p.ready = false;
                    }
                }
            }
            LobbyCmd::Kick(target) => {
                if !is_host || target == id {
                    return;
                }
                if let Some(c) = self.clients.iter().find(|c| c.player.as_ref().is_some_and(|p| p.id == target)) {
                    c.conn.send(&ServerMsg::Kicked("You were removed by the host.".into()));
                    c.conn.close();
                }
            }
            LobbyCmd::Start => {
                if !is_host {
                    return;
                }
                let players = self.lobby_state().players;
                let not_ready: Vec<&str> = players.iter().filter(|p| p.id != id && !p.ready).map(|p| p.name.as_str()).collect();
                if !not_ready.is_empty() {
                    self.system_chat(format!("Waiting for: {}", not_ready.join(", ")));
                    return;
                }
                self.start_match(players);
            }
            LobbyCmd::Chat(_) => {}
        }
        self.broadcast_lobby();
    }

    fn start_match(&mut self, players: Vec<LobbyPlayer>) {
        log::info!("starting match with {} players, seed {}", players.len(), self.settings.seed);
        // Shuffle which home planet each player gets.
        let mut order: Vec<u8> = (0..players.len() as u8).collect();
        let mut rng = Rng::new(self.settings.seed ^ 0x5157);
        for k in (1..order.len()).rev() {
            let r = rng.range_u32(0, k as u32 + 1) as usize;
            order.swap(k, r);
        }
        let inits: Vec<PlayerInit> = players.iter().zip(&order).map(|(p, s)| PlayerInit { id: p.id, slot: *s }).collect();
        let match_players: Vec<MatchPlayer> = players
            .iter()
            .zip(&order)
            .map(|(p, s)| MatchPlayer { id: p.id, name: p.name.clone(), color: p.color, slot: *s })
            .collect();
        let t = Instant::now();
        let sys = Arc::new(system::generate(&self.settings, players.len()));
        let map_hash = sys.hash;
        log::info!("star system generated in {:.3}s (hash {map_hash:016x})", t.elapsed().as_secs_f32());
        let sim = Sim::new(self.content.clone(), sys, &self.settings, &inits);
        for c in &self.clients {
            if let Some(p) = &c.player {
                c.conn.send(&ServerMsg::StartGame(GameStart {
                    settings: self.settings.clone(),
                    players: match_players.clone(),
                    you: p.id,
                    map_hash,
                }));
            }
        }
        self.phase = Phase::Loading { sim: Box::new(sim), ready: HashSet::new(), map_hash, since: Instant::now() };
    }

    fn advance(&mut self) {
        let phase = std::mem::replace(&mut self.phase, Phase::Lobby);
        self.phase = match phase {
            Phase::Lobby => Phase::Lobby,
            Phase::Loading { sim, ready, map_hash, since } => {
                let connected: Vec<PlayerId> = self.members().map(|p| p.id).collect();
                if connected.is_empty() {
                    Phase::Lobby
                } else if connected.iter().all(|id| ready.contains(id)) || since.elapsed() > Duration::from_secs(120) {
                    self.broadcast(&ServerMsg::GameRunning);
                    Phase::Running { sim }
                } else {
                    Phase::Loading { sim, ready, map_hash, since }
                }
            }
            Phase::Running { mut sim } => {
                sim.step();
                for (player, text) in sim.take_notices() {
                    if let Some(c) = self.clients.iter().find(|c| c.player.as_ref().is_some_and(|p| p.id == player)) {
                        c.conn.send(&ServerMsg::Notice(text));
                    }
                }
                if sim.tick.is_multiple_of(SNAPSHOT_EVERY) {
                    self.broadcast(&ServerMsg::Snapshot(Box::new(sim.snapshot())));
                }
                if self.members().count() == 0 {
                    Phase::Lobby
                } else if let Some(winner) = sim.winner {
                    self.broadcast(&ServerMsg::Snapshot(Box::new(sim.snapshot())));
                    self.broadcast(&ServerMsg::GameOver { winner });
                    let name = winner.and_then(|w| self.members().find(|p| p.id == w).map(|p| p.name.clone()));
                    self.system_chat(match name {
                        Some(n) => format!("{n} rules the system!"),
                        None => "The match is over.".into(),
                    });
                    Phase::GameOver { until: Instant::now() + Duration::from_secs(3) }
                } else {
                    Phase::Running { sim }
                }
            }
            Phase::GameOver { until } => {
                if Instant::now() >= until {
                    for c in self.clients.iter_mut() {
                        if let Some(p) = c.player.as_mut() {
                            p.ready = false;
                        }
                    }
                    self.settings.seed = Rng::new(self.settings.seed).next_u64() % 1_000_000_000;
                    self.broadcast_lobby();
                    Phase::Lobby
                } else {
                    Phase::GameOver { until }
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recv(c: &Connection<ClientMsg, ServerMsg>, pred: impl Fn(&ServerMsg) -> bool) -> ServerMsg {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(Incoming::Msg(m)) = c.recv_timeout(Duration::from_millis(100))
                && pred(&m)
            {
                return m;
            }
        }
        panic!("expected message did not arrive");
    }

    /// A whole match over TCP: hello, lobby, start, map hash, snapshots, a command.
    #[test]
    fn tcp_match() {
        let content = Arc::new(ContentDb::embedded());
        let server = start(ServerConfig { bind: "127.0.0.1:0".into(), name: "test".into() }, content.clone()).unwrap();
        let addr = server.addr.to_string();
        let c: Connection<ClientMsg, ServerMsg> = Connection::connect(&addr, Duration::from_secs(2)).unwrap();
        c.send(&ClientMsg::Hello { protocol: PROTOCOL_VERSION, content_hash: content.hash, name: "Tester".into() });
        let ServerMsg::Welcome { you, .. } = recv(&c, |m| matches!(m, ServerMsg::Welcome { .. })) else { unreachable!() };
        c.send(&ClientMsg::Lobby(LobbyCmd::Start));
        let ServerMsg::StartGame(gs) = recv(&c, |m| matches!(m, ServerMsg::StartGame(_))) else { unreachable!() };
        assert_eq!(gs.you, you);
        let sys = system::generate(&gs.settings, gs.players.len());
        assert_eq!(sys.hash, gs.map_hash, "client generates the same system");
        c.send(&ClientMsg::MapReady { map_hash: sys.hash });
        recv(&c, |m| matches!(m, ServerMsg::GameRunning));
        let ServerMsg::Snapshot(s) = recv(&c, |m| matches!(m, ServerMsg::Snapshot(_))) else { unreachable!() };
        assert_eq!(s.units.len(), 4);
        let cc = s.buildings[0].id;
        let drone = content.unit_kind("drone").unwrap();
        c.send(&ClientMsg::Command(GameCommand::Train { building: cc, kind: drone }));
        recv(&c, |m| matches!(m, ServerMsg::Snapshot(s) if s.buildings[0].queue.len() == 1));
    }
}
