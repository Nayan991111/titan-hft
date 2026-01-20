use zerocopy::{IntoBytes, FromBytes, Immutable};

#[repr(C)]
// ADDED "Default" to the derive list below
#[derive(Debug, Clone, Copy, IntoBytes, FromBytes, Immutable, Default)]
pub struct MarketTick {
    pub symbol: [u8; 8],
    pub price: f64,
    pub quantity: u64,
    pub timestamp: u64,
    pub side: u8,
    pub padding: [u8; 7]
}

impl MarketTick {
    pub fn new(sym: &str, price: f64, qty: u64, side: u8) -> Self {
        let mut s = [0u8; 8];
        let bytes = sym.as_bytes();
        let len = bytes.len().min(8);
        s[..len].copy_from_slice(&bytes[..len]);
        
        Self {
            symbol: s,
            price,
            quantity: qty,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            side,
            padding: [0; 7],
        }
    }

    #[inline(always)]
    pub fn symbol_u64(&self) -> u64 {
        u64::from_le_bytes(self.symbol)
    }
}