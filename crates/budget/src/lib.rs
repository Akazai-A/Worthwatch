//! Budgeting. Two sources feed one transaction store:
//!
//! ```text
//!  iPhone Wallet tap ──Shortcut──▶ Tap ─┐
//!                                       ├─▶ reconcile() ──▶ Vec<Txn> ──▶ month() ──▶ budget page
//!  bank CSV / OFX ──parse_statement──▶ CsvRow ─┘      (pure)              (pure)
//! ```
//!
//! Taps are early, provisional copies; the statement is the record. `reconcile` decides
//! which tap and which statement row are the same purchase. Everything here is pure
//! except [`store`], which reads and appends `transactions/YYYY-MM.jsonl`.
//! Amounts on a [`Txn`] are positive for money out (spending) and negative for money in.

pub mod category;
pub mod game;
pub mod month;
pub mod reconcile;
pub mod shortcut;
pub mod statement;
pub mod store;
pub mod tap;

pub use category::{categorize, counts_as_spending, label, CATEGORIES};
pub use game::GameView;
pub use month::{month, BillStatus, CategoryLine, MonthView, UnlinkedCard};
pub use reconcile::{reconcile, Status, Txn};
pub use statement::{parse_statement, rows_for_account, CsvRow, ParsedStatement, Preset, StatementRow};
pub use store::TxnStore;
pub use tap::{parse_taps, Tap, TapParse};

/// FNV-1a, hex. Stable across runs and platforms, which is all an id needs.
pub(crate) fn fnv(parts: &[&str]) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0x1f)) {
            h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}
