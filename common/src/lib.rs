pub mod util;
pub mod order_state;
pub mod order_delegate;

pub use freenet_scaffold::ComposableState;
pub use order_state::*;

// Backward-compat type aliases for new order_state modules
pub type UserId = ed25519_dalek::VerifyingKey;
pub type UserIdKey = ed25519_dalek::VerifyingKey;
