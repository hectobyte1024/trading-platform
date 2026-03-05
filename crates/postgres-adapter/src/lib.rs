//! PostgreSQL Adapter
//!
//! PostgreSQL database integration for the trading platform.
//!
//! Features:
//! - Connection pooling with configurable limits
//! - LedgerStore implementation for double-entry accounting
//! - Database schema initialization
//! - ACID transaction support

pub mod connection;
pub mod ledger_store;

pub use connection::{create_pool, init_schema, PostgresConfig};
pub use ledger_store::PostgresLedgerStore;
