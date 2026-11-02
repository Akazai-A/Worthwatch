//! Market data as the valuation functions see it: manual prices and FX from the
//! ledger merged with whatever the app fetched. Built by the server; pure here.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Currency, Ledger, Symbol};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Manual,
    Fetched,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Market {
    /// symbol -> date -> price in the symbol's currency. A manual price for a date
    /// replaces a fetched one for the same date.
    pub prices: BTreeMap<Symbol, BTreeMap<NaiveDate, (Decimal, Source)>>,
    /// currency -> date -> units of base per 1 unit. Fetched only.
    pub fx: BTreeMap<Currency, BTreeMap<NaiveDate, Decimal>>,
    /// currency -> rate typed in the ledger; fallback when nothing fetched covers the date.
    pub manual_fx: BTreeMap<Currency, Decimal>,
    /// symbol -> currency the provider quoted it in, when known.
    pub currencies: BTreeMap<Symbol, Currency>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct Price {
    pub date: NaiveDate,
    pub price: Decimal,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct Rate {
    /// `None` for a manual (undated) rate.
    pub date: Option<NaiveDate>,
    pub rate: Decimal,
    pub source: Source,
}

impl Market {
    /// Manual data only: what the ledger alone can value.
    pub fn from_ledger(l: &Ledger) -> Self {
        Self::merge(l, &BTreeMap::new(), &BTreeMap::new(), &BTreeMap::new())
    }

    /// Fetched data overlaid with the ledger's manual prices and FX.
    pub fn merge(
        l: &Ledger,
        fetched_prices: &BTreeMap<Symbol, BTreeMap<NaiveDate, Decimal>>,
        fetched_fx: &BTreeMap<Currency, BTreeMap<NaiveDate, Decimal>>,
        currencies: &BTreeMap<Symbol, Currency>,
    ) -> Self {
        let mut prices: BTreeMap<Symbol, BTreeMap<NaiveDate, (Decimal, Source)>> = BTreeMap::new();
        for (sym, days) in fetched_prices {
            let e = prices.entry(sym.clone()).or_default();
            for (d, p) in days {
                e.insert(*d, (*p, Source::Fetched));
            }
        }
        for (sym, days) in &l.prices {
            let e = prices.entry(sym.clone()).or_default();
            for (d, p) in days {
                e.insert(*d, (*p, Source::Manual));
            }
        }
        Market {
            prices,
            fx: fetched_fx.clone(),
            manual_fx: l.settings.fx.clone(),
            currencies: currencies.clone(),
        }
    }

    /// Latest price for a symbol on or before `date`.
    pub fn price_at(&self, symbol: &str, date: NaiveDate) -> Option<Price> {
        self.prices.get(symbol)?.range(..=date).next_back().map(|(d, (p, s))| Price {
            date: *d,
            price: *p,
            source: *s,
        })
    }

    /// The price before the latest one on or before `date`; for day change.
    pub fn previous_price(&self, symbol: &str, date: NaiveDate) -> Option<Price> {
        let mut it = self.prices.get(symbol)?.range(..=date).rev();
        it.next()?;
        it.next().map(|(d, (p, s))| Price { date: *d, price: *p, source: *s })
    }

    /// Rate to convert `currency` into base on `date`: fetched on or before the date,
    /// else the manual rate from the ledger, else `None`.
    pub fn rate_at(&self, base: &str, currency: &str, date: NaiveDate) -> Option<Rate> {
        if currency == base {
            return Some(Rate { date: None, rate: Decimal::ONE, source: Source::Manual });
        }
        if let Some((d, r)) = self.fx.get(currency).and_then(|m| m.range(..=date).next_back()) {
            return Some(Rate { date: Some(*d), rate: *r, source: Source::Fetched });
        }
        self.manual_fx.get(currency).map(|r| Rate { date: None, rate: *r, source: Source::Manual })
    }

    /// Most recent date any fetched price was quoted for, across all symbols.
    pub fn latest_fetched_price_date(&self) -> Option<NaiveDate> {
        self.prices
            .values()
            .flat_map(|m| m.iter().filter(|(_, (_, s))| *s == Source::Fetched).map(|(d, _)| *d))
            .max()
    }
}
