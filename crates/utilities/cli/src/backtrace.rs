//! Helper to set the backtrace env var.

use std::sync::Once;

static INIT: Once = Once::new();

/// Sets the RUST_BACKTRACE environment variable to 1 if it is not already set.
/// This function is thread-safe and will only set the variable once.
pub fn enable() {
    INIT.call_once(|| {
        // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
        if std::env::var_os("RUST_BACKTRACE").is_none() {
            // Use std::env::set_var safely - the Once ensures this only happens once
            std::env::set_var("RUST_BACKTRACE", "1");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_backtrace_enable() {
        // Clear the environment variable first
        std::env::remove_var("RUST_BACKTRACE");
        
        // Enable backtrace
        enable();
        
        // Check that the variable was set
        assert_eq!(std::env::var("RUST_BACKTRACE").unwrap(), "1");
    }

    #[test]
    fn test_backtrace_thread_safety() {
        // Clear the environment variable first
        std::env::remove_var("RUST_BACKTRACE");
        
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        
        // Spawn multiple threads that all call enable()
        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            let handle = thread::spawn(move || {
                enable();
                counter.fetch_add(1, Ordering::SeqCst);
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify that all threads completed
        assert_eq!(counter.load(Ordering::SeqCst), 10);
        
        // Verify that the environment variable was set only once
        assert_eq!(std::env::var("RUST_BACKTRACE").unwrap(), "1");
    }

    #[test]
    fn test_backtrace_preserves_existing_value() {
        // Set a custom value
        std::env::set_var("RUST_BACKTRACE", "full");
        
        // Call enable() - should not change the existing value
        enable();
        
        // Verify the value was preserved
        assert_eq!(std::env::var("RUST_BACKTRACE").unwrap(), "full");
    }
}
