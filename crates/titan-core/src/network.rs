use crate::buffer::RingBuffer;
use titan_common::MarketTick;
use socket2::{Socket, Domain, Type, Protocol};
use std::net::SocketAddr;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;

// CONFIGURATION
const TICKS_PER_PACKET: usize = 32;
const TICK_SIZE: usize = std::mem::size_of::<MarketTick>();
const PACKET_SIZE: usize = TICK_SIZE * TICKS_PER_PACKET;
const BATCH_READ_LOOPS: usize = 16; 

pub struct BatchReceiver {
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
        println!(">>> [NETWORK] Jumbo Receiver initialized (32 ticks/pkt).");
        Ok(Self { socket, buffer: [0u8; PACKET_SIZE] })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn listen_loop(&mut self, ring_buffer: &RingBuffer) {
        println!(">>> [NETWORK] Starting Jumbo Packet Processing...");
        
        // Correct: Buffer of MaybeUninit
        let mut buf = [MaybeUninit::<u8>::uninit(); PACKET_SIZE];
        
        loop {
            let mut received_any = false;
            
            for _ in 0..BATCH_READ_LOOPS {
                // FIXED: Pass the MaybeUninit buffer directly
                match self.socket.recv(&mut buf) {
                    Ok(size) => {
                        if size == PACKET_SIZE {
                            received_any = true;
                            
                            // Interpret the initialized bytes as MarketTicks
                            let ticks = unsafe { 
                                std::slice::from_raw_parts(
                                    buf.as_ptr() as *const MarketTick, 
                                    TICKS_PER_PACKET
                                ) 
                            };

                            // Push all 32 ticks to RingBuffer
                            for tick in ticks {
                                while !ring_buffer.write(*tick) { std::hint::spin_loop(); }
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