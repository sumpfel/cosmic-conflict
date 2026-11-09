//! Minimal, dependency-free TCP transport: one reader and one writer thread per connection,
//! exchanging length-prefixed postcard frames through channels.
//!
//! Kept behind this small API so a UDP/QUIC transport can replace it later without touching
//! the server or client logic.

use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use serde::{de::DeserializeOwned, Serialize};

/// Frames larger than this are rejected (protects against garbage input).
pub const MAX_FRAME: usize = 32 * 1024 * 1024;

pub enum Incoming<T> {
    Msg(T),
    Closed(String),
}

/// A bidirectional message connection. `O` = messages we send, `I` = messages we receive.
pub struct Connection<O, I> {
    out: Sender<Vec<u8>>,
    inc: Receiver<Incoming<I>>,
    alive: Arc<AtomicBool>,
    stream: TcpStream,
    pub peer: SocketAddr,
    _o: PhantomData<fn(O)>,
}

fn read_frame(s: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("frame too large: {n}")));
    }
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

impl<O, I> Connection<O, I>
where
    O: Serialize,
    I: DeserializeOwned + Send + 'static,
{
    /// Wraps an established stream and starts the I/O threads.
    pub fn from_stream(stream: TcpStream) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        let peer = stream.peer_addr()?;
        let alive = Arc::new(AtomicBool::new(true));
        let (out_tx, out_rx) = unbounded::<Vec<u8>>();
        let (in_tx, in_rx) = unbounded::<Incoming<I>>();

        let mut rs = stream.try_clone()?;
        let alive_r = alive.clone();
        thread::Builder::new().name(format!("net-read-{peer}")).spawn(move || {
            loop {
                match read_frame(&mut rs) {
                    Ok(buf) => match postcard::from_bytes::<I>(&buf) {
                        Ok(m) => {
                            if in_tx.send(Incoming::Msg(m)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = in_tx.send(Incoming::Closed(format!("bad message: {e}")));
                            break;
                        }
                    },
                    Err(e) => {
                        let reason = if e.kind() == io::ErrorKind::UnexpectedEof {
                            "connection closed".to_string()
                        } else {
                            e.to_string()
                        };
                        let _ = in_tx.send(Incoming::Closed(reason));
                        break;
                    }
                }
            }
            alive_r.store(false, Ordering::SeqCst);
        })?;

        let mut ws = stream.try_clone()?;
        let alive_w = alive.clone();
        thread::Builder::new().name(format!("net-write-{peer}")).spawn(move || {
            while let Ok(frame) = out_rx.recv() {
                let len = (frame.len() as u32).to_le_bytes();
                if ws.write_all(&len).and_then(|_| ws.write_all(&frame)).is_err() {
                    break;
                }
            }
            alive_w.store(false, Ordering::SeqCst);
            let _ = ws.shutdown(Shutdown::Both);
        })?;

        Ok(Connection { out: out_tx, inc: in_rx, alive, stream, peer, _o: PhantomData })
    }

    /// Connects to a server ("host:port").
    pub fn connect(addr: &str, timeout: Duration) -> io::Result<Self> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "address did not resolve");
        for a in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&a, timeout) {
                Ok(s) => return Self::from_stream(s),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Queues a message. Returns false if the connection is closed.
    pub fn send(&self, msg: &O) -> bool {
        match postcard::to_allocvec(msg) {
            Ok(bytes) => self.send_raw(bytes),
            Err(_) => false,
        }
    }

    /// Queues an already encoded message (used to encode once for many receivers).
    pub fn send_raw(&self, bytes: Vec<u8>) -> bool {
        self.is_alive() && self.out.send(bytes).is_ok()
    }

    pub fn try_recv(&self) -> Option<Incoming<I>> {
        match self.inc.try_recv() {
            Ok(m) => Some(m),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Incoming::Closed("disconnected".into())),
        }
    }

    pub fn recv_timeout(&self, t: Duration) -> Option<Incoming<I>> {
        self.inc.recv_timeout(t).ok()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Number of frames waiting to be written (back-pressure indicator).
    pub fn pending(&self) -> usize {
        self.out.len()
    }

    pub fn close(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl<O, I> Drop for Connection<O, I> {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// Encodes a message once (for broadcasting to many connections).
pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    postcard::to_allocvec(msg).unwrap_or_default()
}

/// Accepts connections on a background thread.
pub struct Listener<O, I> {
    rx: Receiver<Connection<O, I>>,
    pub local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl<O, I> Listener<O, I>
where
    O: Serialize + 'static,
    I: DeserializeOwned + Send + 'static,
{
    pub fn bind(addr: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        thread::Builder::new().name("net-accept".into()).spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((s, _)) => {
                        let _ = s.set_nonblocking(false);
                        if let Ok(c) = Connection::from_stream(s)
                            && tx.send(c).is_err() {
                                break;
                            }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(100)),
                }
            }
        })?;
        Ok(Listener { rx, local_addr, stop })
    }

    pub fn try_accept(&self) -> Option<Connection<O, I>> {
        self.rx.try_recv().ok()
    }
}

impl<O, I> Drop for Listener<O, I> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback() {
        let l: Listener<String, String> = Listener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr.to_string();
        let c: Connection<String, String> = Connection::connect(&addr, Duration::from_secs(2)).unwrap();
        let mut server = None;
        for _ in 0..100 {
            if let Some(s) = l.try_accept() {
                server = Some(s);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let s = server.expect("accepted");
        assert!(c.send(&"hello".to_string()));
        match s.recv_timeout(Duration::from_secs(2)) {
            Some(Incoming::Msg(m)) => assert_eq!(m, "hello"),
            _ => panic!("no message"),
        }
        assert!(s.send(&"world".to_string()));
        match c.recv_timeout(Duration::from_secs(2)) {
            Some(Incoming::Msg(m)) => assert_eq!(m, "world"),
            _ => panic!("no reply"),
        }
    }
}
