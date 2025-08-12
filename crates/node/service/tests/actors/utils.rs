use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

use rand::{RngCore, SeedableRng};

pub(crate) static SEED_GENERATOR_BUILDER: LazyLock<SeedGeneratorBuilder> =
    LazyLock::new(SeedGeneratorBuilder::new);

pub(crate) struct SeedGeneratorBuilder(AtomicU64);

impl SeedGeneratorBuilder {
    pub(crate) const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn next_generator(&self) -> SeedGenerator {
        SeedGenerator(rand::rngs::StdRng::seed_from_u64(self.next()))
    }
}

pub(crate) struct SeedGenerator(rand::rngs::StdRng);

impl SeedGenerator {
    pub(crate) fn next_loopback_address(&mut self) -> IpAddr {
        let next_u64 = self.0.next_u64();

        IpAddr::V4(Ipv4Addr::new(
            127,
            next_u64 as u8,
            (next_u64 >> 8) as u8,
            (next_u64 >> 16) as u8,
        ))
    }

    pub(crate) fn next_port(&mut self) -> u16 {
        let next_u32 = self.0.next_u32();

        next_u32 as u16
    }
}
