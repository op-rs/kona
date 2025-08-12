use std::{
    net::{IpAddr, Ipv4Addr},
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

/// Static counter for generating unique loopback addresses.
/// Each increment gives us a unique combination of the last 3 bytes.
static ADDRESS_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Static counter for generating unique ports.
/// Starts at 10000 to avoid common port ranges.
static PORT_COUNTER: AtomicU32 = AtomicU32::new(10000);

/// Static counter for generating unique chain IDs.
static CHAIN_ID_COUNTER: AtomicU64 = AtomicU64::new(1000);

/// Thread-safe allocator for test network addresses and ports.
/// Uses atomics to ensure unique address/port allocation across parallel tests.
pub(crate) struct NetworkAllocator;

impl NetworkAllocator {
    /// Allocates a unique loopback address for testing.
    /// Returns addresses in the 127.x.x.x range.
    pub(crate) fn next_loopback_address() -> IpAddr {
        let counter = ADDRESS_COUNTER.fetch_add(1, Ordering::SeqCst);

        IpAddr::V4(Ipv4Addr::new(
            127,
            (counter & 0xFF) as u8,
            ((counter >> 8) & 0xFF) as u8,
            ((counter >> 16) & 0xFF) as u8,
        ))
    }

    /// Allocates a unique port for testing.
    /// Wraps around at 65535 back to 10000.
    pub(crate) fn next_port() -> u16 {
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);

        // Wrap around if we exceed u16::MAX, staying above 10000
        if port > 65535 {
            PORT_COUNTER.store(10000, Ordering::SeqCst);
            10000
        } else {
            port as u16
        }
    }

    /// Allocates a unique chain ID for testing.
    pub(crate) fn next_chain_id() -> u64 {
        CHAIN_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    }
}
