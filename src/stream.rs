use std::io::{self, Read, Write};
use std::mem;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::rc::Rc;

use futures::{Async, Future, Poll};
use kcp::Kcp;
use rand;
use tokio_core::net::UdpSocket;
use tokio_core::reactor::{Handle, PollEvented};
use tokio_io::{AsyncRead, AsyncWrite};

use config::KcpConfig;
use kcp_io::{KcpIo, KcpIoMode};
use skcp::{KcpOutput, SharedKcp};

/// KCP client for interacting with server
pub struct KcpClientStream {
    udp: Rc<UdpSocket>,
    io: PollEvented<KcpIo>,
}

impl Read for KcpClientStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut pkg: [u8; 65536] = unsafe { mem::zeroed() };

        if let Ok((n, addr)) = self.udp.recv_from(&mut pkg) {
            trace!("[RECV] UDP {} size={} {:?}", addr, n, &pkg[..n]);
            self.io.get_mut().input_buf(&pkg[..n])?;
        }
        self.io.read(buf)
    }
}

impl AsyncRead for KcpClientStream {}

impl AsyncWrite for KcpClientStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }
}

impl Write for KcpClientStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.get_mut().set_writable()?;
        self.io.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

/// Future for creating a new `KcpClientStream`
pub struct KcpStreamNew {
    addr: SocketAddr,
    handle: Handle,
    config: KcpConfig,
}

impl Future for KcpStreamNew {
    type Item = KcpClientStream;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<KcpClientStream, io::Error> {
        let local = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 0);

        let udp = UdpSocket::bind(&local, &self.handle)?;
        let udp = Rc::new(udp);

        let mut kcp = Kcp::new(rand::random::<u32>(), KcpOutput::new(udp.clone(), self.addr));
        self.config.apply_config(&mut kcp);
        let shared_kcp = SharedKcp::new(kcp);

        let io = KcpIo::new(shared_kcp, self.addr, &self.handle, None, KcpIoMode::Client)?;
        let io = PollEvented::new(io, &self.handle)?;
        let stream = KcpClientStream { udp: udp, io: io };
        Ok(Async::Ready(stream))
    }
}

/// KCP client between a local and remote socket
///
/// After creating a `KcpStream` by either connecting to a remote host or accepting a connection on a `KcpListener`,
/// data can be transmitted by reading and writing to it.
pub struct KcpStream {
    io: PollEvented<KcpIo>,
}

impl KcpStream {
    #[doc(hidden)]
    pub fn new(io: PollEvented<KcpIo>) -> KcpStream {
        KcpStream { io: io }
    }

    /// Opens a KCP connection to a remote host.
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> KcpStreamNew {
        KcpStream::connect_with_config(addr, handle, KcpConfig::default())
    }

    /// Opens a KCP connection to a remote host.
    pub fn connect_with_config(addr: &SocketAddr, handle: &Handle, config: KcpConfig) -> KcpStreamNew {
        KcpStreamNew {
            addr: *addr,
            handle: handle.clone(),
            config: config,
        }
    }

    #[doc(hidden)]
    pub fn input_buf(&mut self, buf: &[u8]) -> io::Result<()> {
        let io = self.io.get_mut();
        io.input_buf(buf)
    }
}

impl Read for KcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read(buf)
    }
}

impl Write for KcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.get_mut().set_writable()?;
        self.io.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl AsyncRead for KcpStream {}

impl AsyncWrite for KcpStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }
}
