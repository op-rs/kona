//! Helper macros for the engine crate.

/// A macro that loops over a sender channel, sending the message repeatedly until success.
/// Each failure is logged with [tracing::warn], using the specified target string.
#[macro_export]
macro_rules! send_until_success {
    ($target:expr, $sender:expr, $message:expr) => {
        loop {
            match $sender.send($message.clone()) {
                Ok(_) => break,
                Err(e) => {
                    tracing::warn!(target: $target, "Failed to send message: {:?}", e);
                }
            }
        }
    };
}

/// Wraps the `send_until_success!` macro, accepting an optional sender channel.
/// Only sends the message if the sender is `Some`.
#[macro_export]
macro_rules! send_until_success_opt {
    ($target:expr, $sender:expr, $message:expr) => {
        if let Some(sender) = $sender {
            $crate::send_until_success!($target, sender, $message);
        }
    };
}
