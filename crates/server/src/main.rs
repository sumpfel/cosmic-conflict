use std::sync::atomic::{AtomicU64, Ordering};

use cosmic_shared::{ClientMessage, DEFAULT_PORT, ServerMessage, decode, encode};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{DEFAULT_PORT}");
    let listener = TcpListener::bind(&addr).await?;
    println!("cosmic-server listening on {addr}");

    let (tx, _) = broadcast::channel::<(u64, ServerMessage)>(256);
    loop {
        let (stream, peer) = listener.accept().await?;
        println!("connection from {peer}");
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, tx).await {
                eprintln!("client {peer} error: {e}");
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    tx: broadcast::Sender<(u64, ServerMessage)>,
) -> std::io::Result<()> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut rx = tx.subscribe();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    write_msg(&mut write, &ServerMessage::Welcome { id }).await?;

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { break };
                match decode::<ClientMessage>(&line) {
                    Ok(ClientMessage::Join { name }) => {
                        let _ = tx.send((id, ServerMessage::PlayerJoined { id, name }));
                    }
                    Ok(ClientMessage::Move { position }) => {
                        let _ = tx.send((id, ServerMessage::PlayerMoved { id, position }));
                    }
                    Err(e) => eprintln!("bad message from {id}: {e}"),
                }
            }
            Ok((from, msg)) = rx.recv() => {
                if from != id {
                    write_msg(&mut write, &msg).await?;
                }
            }
        }
    }

    let _ = tx.send((id, ServerMessage::PlayerLeft { id }));
    Ok(())
}

async fn write_msg(
    write: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &ServerMessage,
) -> std::io::Result<()> {
    let line = encode(msg).map_err(std::io::Error::other)?;
    write.write_all(line.as_bytes()).await
}
