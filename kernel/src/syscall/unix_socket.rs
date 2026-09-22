//! UNIX Domain Socket (AF_UNIX / AF_LOCAL) Subsystem
//!
//! Provides in-memory bidirectional stream sockets for local inter-process
//! communication (e.g. X11 client/server communication between Xfbdev and flwm/wbar/aterm).

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

use crate::task::pipe::{PipeBuffer, PipeError};

pub const AF_UNIX: i32 = 1;
pub const AF_LOCAL: i32 = 1;
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;

pub static NEXT_SOCKET_ID: AtomicUsize = AtomicUsize::new(100);

pub static UNIX_SOCKET_REGISTRY: Mutex<BTreeMap<String, Arc<Mutex<UnixSocket>>>> =
    Mutex::new(BTreeMap::new());

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SockAddrUn {
    pub sun_family: u16,
    pub sun_path: [u8; 108],
}

impl SockAddrUn {
    pub fn get_path(&self) -> String {
        let mut len = 0;
        while len < self.sun_path.len() && self.sun_path[len] != 0 {
            len += 1;
        }
        String::from_utf8_lossy(&self.sun_path[..len]).to_string()
    }
}

#[derive(Clone)]
pub enum UnixSocketState {
    Unbound,
    Bound(String),
    Listening {
        backlog: usize,
        pending_queue: Vec<Arc<Mutex<UnixSocket>>>,
    },
    Connected {
        peer: Option<Arc<Mutex<UnixSocket>>>,
        rx_buffer: Arc<Mutex<PipeBuffer>>,
        tx_buffer: Arc<Mutex<PipeBuffer>>,
    },
    Closed,
}

pub struct UnixSocket {
    pub id: usize,
    pub domain: i32,
    pub socket_type: i32,
    pub protocol: i32,
    pub path: Option<String>,
    pub state: UnixSocketState,
    pub flags: u32,
    /// File descriptors (across all processes) that refer to this socket. When the last one
    /// closes, the peer sees end-of-file / EPIPE.
    pub open_fds: usize,
}

impl UnixSocket {
    pub fn new(domain: i32, socket_type: i32, protocol: i32) -> Arc<Mutex<Self>> {
        let id = NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed);
        Arc::new(Mutex::new(Self {
            id,
            domain,
            socket_type,
            protocol,
            path: None,
            state: UnixSocketState::Unbound,
            flags: 0,
            open_fds: 1,
        }))
    }

    pub fn bind(self_arc: &Arc<Mutex<Self>>, path: &str) -> isize {
        let mut sock = self_arc.lock();
        if !matches!(sock.state, UnixSocketState::Unbound) {
            return -22; // -EINVAL
        }

        let clean_path = path.trim();
        let mut reg = UNIX_SOCKET_REGISTRY.lock();
        if reg.contains_key(clean_path) {
            return -98; // -EADDRINUSE
        }

        reg.insert(clean_path.to_string(), self_arc.clone());
        sock.path = Some(clean_path.to_string());
        sock.state = UnixSocketState::Bound(clean_path.to_string());

        crate::lunix_serial_println!("  [AF_UNIX] Bound socket #{} to '{}'", sock.id, clean_path);
        0
    }

    pub fn listen(&mut self, backlog: usize) -> isize {
        match self.state {
            UnixSocketState::Bound(_) => {
                self.state = UnixSocketState::Listening {
                    backlog,
                    pending_queue: Vec::new(),
                };
                crate::lunix_serial_println!("  [AF_UNIX] Socket #{} listening (backlog={})", self.id, backlog);
                0
            }
            _ => -22, // -EINVAL
        }
    }

    pub fn connect(client_arc: &Arc<Mutex<Self>>, target_path: &str) -> isize {
        let clean_path = target_path.trim();
        let listener_opt = {
            let reg = UNIX_SOCKET_REGISTRY.lock();
            reg.get(clean_path).cloned()
        };

        if let Some(listener_arc) = listener_opt {
            let mut listener = listener_arc.lock();
            if let UnixSocketState::Listening { ref mut pending_queue, backlog } = listener.state {
                if pending_queue.len() >= backlog && backlog > 0 {
                    return -111; // -ECONNREFUSED
                }

                // Create duplex pipe streams
                let s2c = Arc::new(Mutex::new(PipeBuffer::new()));
                let c2s = Arc::new(Mutex::new(PipeBuffer::new()));

                let server_peer = Arc::new(Mutex::new(UnixSocket {
                    id: NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed),
                    domain: AF_UNIX,
                    socket_type: SOCK_STREAM,
                    protocol: 0,
                    path: Some(clean_path.to_string()),
                    state: UnixSocketState::Connected {
                        peer: Some(client_arc.clone()),
                        rx_buffer: c2s.clone(), // Server reads client data
                        tx_buffer: s2c.clone(), // Server writes to client
                    },
                    flags: 0,
                    open_fds: 1,
                }));

                pending_queue.push(server_peer.clone());

                let mut client = client_arc.lock();
                client.path = Some(clean_path.to_string());
                client.state = UnixSocketState::Connected {
                    peer: Some(server_peer),
                    rx_buffer: s2c, // Client reads server data
                    tx_buffer: c2s, // Client writes to server
                };

                crate::lunix_serial_println!(
                    "  [AF_UNIX] Connected client socket #{} to listener '{}'",
                    client.id, clean_path
                );
                0
            } else {
                -111 // -ECONNREFUSED
            }
        } else {
            -2 // -ENOENT
        }
    }

    pub fn accept(self_arc: &Arc<Mutex<Self>>) -> Result<Arc<Mutex<Self>>, isize> {
        let mut sock = self_arc.lock();
        if let UnixSocketState::Listening { ref mut pending_queue, .. } = sock.state {
            if let Some(conn) = pending_queue.pop() {
                crate::lunix_serial_println!("  [AF_UNIX] Accepted connection on socket #{}", sock.id);
                Ok(conn)
            } else {
                Err(-11) // -EWOULDBLOCK / -EAGAIN
            }
        } else {
            Err(-22) // -EINVAL
        }
    }

    /// One descriptor for this socket was closed. The last close hangs up the connection.
    pub fn close_one(&mut self) {
        self.open_fds = self.open_fds.saturating_sub(1);
        if self.open_fds == 0 {
            if let UnixSocketState::Connected { ref rx_buffer, ref tx_buffer, .. } = self.state {
                tx_buffer.lock().remove_writer(); // the peer reads EOF once drained
                rx_buffer.lock().remove_reader(); // the peer's writes now fail with EPIPE
            }
        }
    }

    pub fn read(&self, buf: &mut [u8], non_blocking: bool) -> isize {
        if buf.is_empty() {
            return 0;
        }

        if let UnixSocketState::Connected { ref rx_buffer, .. } = self.state {
            loop {
                let res = {
                    let mut pipe = rx_buffer.lock();
                    pipe.read(buf, non_blocking)
                };
                match res {
                    Ok(n) => return n as isize,
                    Err(PipeError::WouldBlock) => {
                        if non_blocking {
                            return -11; // -EAGAIN
                        }
                        crate::task::scheduler::sleep_ms(1);
                    }
                    Err(PipeError::BrokenPipe) => return 0, // EOF
                    Err(_) => return -5, // -EIO
                }
            }
        } else {
            -107 // -ENOTCONN
        }
    }

    pub fn write(&self, buf: &[u8], non_blocking: bool) -> isize {
        if buf.is_empty() {
            return 0;
        }

        if let UnixSocketState::Connected { ref tx_buffer, .. } = self.state {
            loop {
                let res = {
                    let mut pipe = tx_buffer.lock();
                    pipe.write(buf, non_blocking)
                };
                match res {
                    Ok(n) => return n as isize,
                    Err(PipeError::WouldBlock) | Err(PipeError::BufferFull) => {
                        if non_blocking {
                            return -11; // -EAGAIN
                        }
                        crate::task::scheduler::sleep_ms(1);
                    }
                    Err(PipeError::BrokenPipe) => return -32, // -EPIPE
                }
            }
        } else {
            -107 // -ENOTCONN
        }
    }

    pub fn poll_read_ready(&self) -> bool {
        match self.state {
            UnixSocketState::Listening { ref pending_queue, .. } => !pending_queue.is_empty(),
            UnixSocketState::Connected { ref rx_buffer, .. } => {
                let pipe = rx_buffer.lock();
                // Readable when data is waiting, or when the peer hung up (read returns 0).
                pipe.available_to_read() > 0 || pipe.writers_count == 0 || pipe.closed_write
            }
            _ => false,
        }
    }

    pub fn poll_write_ready(&self) -> bool {
        match self.state {
            UnixSocketState::Connected { ref tx_buffer, .. } => {
                let pipe = tx_buffer.lock();
                pipe.available_to_write() > 0
            }
            _ => false,
        }
    }
}
