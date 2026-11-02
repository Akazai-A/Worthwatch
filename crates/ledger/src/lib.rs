//! The ledger is the single source of truth: a YAML file the user owns.
//! Every change goes through [`Ledger::apply`]; nothing else writes the file.

pub mod io;
pub mod mutation;
pub mod schema;
pub mod validate;

pub use mutation::Mutation;
pub use schema::*;
pub use validate::{Diagnostic, LedgerError};
