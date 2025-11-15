// Core business logic (shared across all platforms)
pub mod core;

// Cloudflare Workers implementation
#[cfg(feature = "cloudflare")]
mod cloudflare_worker;

#[cfg(feature = "cloudflare")]
pub use cloudflare_worker::*;
