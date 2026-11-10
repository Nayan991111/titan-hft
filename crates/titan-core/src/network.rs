use crate::buffer::RingBuffer;
use titan_common::MarketTick;
use socket2::{Socket, Domain, Type, Protocol};
use std::net::SocketAddr;
use std::io;
#[cfg(not(target_os = "linux"))]
use std::mem::MaybeUninit;
#[cfg(not(target_os = "linux"))]
use zerocopy::FromBytes;
use std::os::unix::io::AsRawFd;

// CONFIGURATION
const TICKS_PER_PACKET: usize = 32;
const TICK_SIZE: usize = std::mem::size_of::<MarketTick>();
const PACKET_SIZE: usize = TICK_SIZE * TICKS_PER_PACKET;
#[cfg(not(target_os = "linux"))]
const BATCH_READ_LOOPS: usize = 16;

pub struct BatchReceiver {
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    socket: Socket,
    #[allow(dead_code)]
    buffer: [u8; PACKET_SIZE], 
}

impl BatchReceiver {
    pub fn new(addr: SocketAddr) -> io::Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        let _ = socket.set_recv_buffer_size(8 * 1024 * 1024); // 8MB OS Buffer

        #[cfg(not(target_os = "windows"))]
        {
            let fd = socket.as_raw_fd();
            unsafe {
                let optval: libc::c_int = 1;
                libc::setsockopt(
                    fd, 
                    libc::SOL_SOCKET, 
                    libc::SO_REUSEPORT, 
                    &optval as *const _ as *const libc::c_void, 
                    std::mem::size_of_val(&optval) as libc::socklen_t
                );
            }
        }

        socket.bind(&addr.into())?;
        println!(">>> [NETWORK] UDP receiver bound (32 ticks per datagram).");
        Ok(Self { socket, buffer: [0u8; PACKET_SIZE] })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn listen_loop(&mut self, ring_buffer: &RingBuffer) {
        println!(">>> [NETWORK] Receiving...");
        
        let mut buf = [MaybeUninit::<u8>::uninit(); PACKET_SIZE];
        
        loop {
            let mut received_any = false;
            
            for _ in 0..BATCH_READ_LOOPS {
                match self.socket.recv(&mut buf) {
                    Ok(size) => {
                        if size == PACKET_SIZE {
                            received_any = true;
                            
                            // SAFETY: recv initialised the first `size` bytes.
                            let bytes = unsafe {
                                std::slice::from_raw_parts(buf.as_ptr() as *const u8, size)
                            };
                            // read_from_bytes copies each tick out, so the receive
                            // buffer's alignment (1) does not matter.
                            for chunk in bytes.chunks_exact(TICK_SIZE) {
                                if let Ok(tick) = MarketTick::read_from_bytes(chunk) {
                                    while !ring_buffer.write(tick) { std::hint::spin_loop(); }
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { break; }
                    Err(_) => { break; } 
                }
            }
            
            if !received_any {
                std::hint::spin_loop();
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    pub fn listen_loop(&mut self, _ring: &RingBuffer) {}
}