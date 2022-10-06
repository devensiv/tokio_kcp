use std::io::ErrorKind;
use std::net::SocketAddr;

use log::{info, trace};
use tokio::io::{stdin, AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::select;
use tokio_kcp::{KcpConfig, KcpStream};

const HELLO: [u8; 6] = *b"HELLLO";

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = KcpConfig::default();

    let server_addr = "127.0.0.1:3100".parse::<SocketAddr>().unwrap();
    let sock = match UdpSocket::bind(server_addr).await {
        Ok(sock) => sock,
        Err(err) => match err.kind() {
            ErrorKind::AddrInUse => run_client(server_addr, config).await,
            _ => panic!("Unexpected error: {}", err),
        },
    };

    trace!("Running as server");
    let mut buf = [0u8; 6];
    let (_, addr) = sock.recv_from(&mut buf).await.unwrap();
    sock.connect(addr).await.unwrap();

    if buf != HELLO {
        panic!("Hello not received");
    }

    let mut stream = KcpStream::connect_with_socket(&config, sock, addr).await.unwrap();

    let mut buffer = [0u8; 8192];
    let mut buffer2 = [0u8; 8192];
    let mut i = stdin();
    loop {
        select! {
            Ok(n) = i.read(&mut buffer) => stream.write_all(&buffer[..n]).await.unwrap(),
            Ok(n) = stream.read(&mut buffer2) =>info!("{}", String::from_utf8_lossy(&buffer2[..n-1])),
        }
    }
}

async fn run_client(addr: SocketAddr, conf: KcpConfig) -> ! {
    trace!("Running as client");
    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    sock.connect(addr).await.unwrap();

    sock.send(&HELLO).await.unwrap();

    let addr = sock.peer_addr().unwrap();

    let mut stream = KcpStream::connect_with_socket(&conf, sock, addr).await.unwrap();

    let mut buffer = [0u8; 8192];
    let mut buffer2 = [0u8; 8192];
    let mut i = stdin();
    loop {
        select! {
            Ok(n) = i.read(&mut buffer) => stream.write_all(&buffer[..n]).await.unwrap(),
            Ok(n) = stream.read(&mut buffer2) => info!("{}", String::from_utf8_lossy(&buffer2[..n-1])),
        }
    }
}
