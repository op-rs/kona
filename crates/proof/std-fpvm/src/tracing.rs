//! This module contains

use crate::io;
use alloc::{
    format,
    string::{String, ToString},
};
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    Event, Level, Metadata, Subscriber,
};

/// Custom [Subscriber] implementation that uses [crate::io] to write log entries to
/// [crate::FileDescriptor::StdOut].
#[derive(Debug, Clone)]
pub struct FpvmTracingSubscriber {
    min_level: Level,
}

impl FpvmTracingSubscriber {
    /// Create a new [FpvmTracingSubscriber] with the specified minimum log level.
    pub const fn new(min_level: Level) -> Self {
        Self { min_level }
    }
}

impl Subscriber for FpvmTracingSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        // Comparisons for the [Level] type are inverted. See the [Level] documentation for more
        // information.
        if *metadata.level() > self.min_level {
            return;
        }

        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);
        io::print(&format!("[{}] {}: {}", metadata.level(), metadata.target(), visitor.message));
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// Custom [Visit] implementation to extract log  field values.
struct FieldVisitor {
    message: String,
}

impl FieldVisitor {
    const fn new() -> Self {
        Self { message: String::new() }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, _field: &Field, value: &dyn core::fmt::Debug) {
        self.message = format!("{:?}", value);
    }

    fn record_str(&mut self, _field: &Field, value: &str) {
        self.message = value.to_string();
    }
}
