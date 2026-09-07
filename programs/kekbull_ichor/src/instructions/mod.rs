pub mod admin;
pub mod convert;
pub mod fees;
pub mod initialize;

// Glob so `#[derive(Accounts)]` generated modules (`__client_accounts_*`,
// `__cpi_client_accounts_*`) reach crate root via `pub use instructions::*`.
pub use admin::*;
pub use convert::*;
pub use fees::*;
pub use initialize::*;
