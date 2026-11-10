use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};
use titan_common::MarketTick;

// Wire format.
const TICKS_PER_PACKET: usize = 32;
const PACKET_SIZE: usize = std::mem::size_of::<MarketTick>() * TICKS_PER_PACKET;

fn main() {
    println!("==================================================");
    println!("   TITAN FEED SIMULATOR (UDP, 32 ticks/datagram)");
    println!("   Mode: 32 Ticks Per Packet (MTU Safe)           ");
    println!("   Scenario: CROSSING MARKETS (Force Execution)   ");
    println!("==================================================");

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind simulator socket");
    socket.connect("127.0.0.1:5555").expect("Failed to connect to engine");

    println!(">>> [FEED] Generating batches...");

    // Pre-build all datagrams so generation cost is outside the send loop.
    let total_orders = 10_000_000; // 10 Million orders
    let total_packets = total_orders / TICKS_PER_PACKET;
    
    // One buffer per datagram, 32 ticks x 40 bytes = 1280 bytes.
    let mut packets: Vec<Vec<u8>> = Vec::with_capacity(total_packets);

    for i in 0..total_packets {
        let mut batch_buffer = Vec::with_capacity(PACKET_SIZE);
        
        for j in 0..TICKS_PER_PACKET {
            let id = (i * TICKS_PER_PACKET + j) as u64;
            let side = if id % 2 == 0 { 1 } else { 2 };
            
            // Both sides quote 100.0 so incoming orders cross.
            let price = 100.0;
            
            let mut tick = MarketTick::new("AAPL", price, 10, side);
            tick.timestamp = id;

            // Serialize struct to bytes raw
            let tick_bytes = unsafe {
                std::slice::from_raw_parts(
                    (&tick as *const MarketTick) as *const u8,
                    std::mem::size_of::<MarketTick>()
                )
            };
            batch_buffer.extend_from_slice(tick_bytes);
        }
        packets.push(batch_buffer);
    }

    println!(">>> [FEED] {} datagrams ready. Sending in 3 s...", packets.len());
    thread::sleep(Duration::from_secs(3));
    println!(">>> [FEED] FIRE!");

    let start = Instant::now();
    let mut packets_sent = 0;

    loop {
        for packet in &packets {
            match socket.send(packet) {
                Ok(_) => packets_sent += 1,
                Err(_) => continue,
            }
        }

        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 1.0 {
            let orders_sent = packets_sent as f64 * TICKS_PER_PACKET as f64;
            println!(">>> [FEED] Sent: {:.2} M Orders | Rate: {:.2} M/sec", 
                orders_sent / 1_000_000.0, 
                orders_sent / elapsed / 1_000_000.0
            );
            // Stop after 5 s.
            if elapsed > 5.0 { break; } 
        }
    }
}