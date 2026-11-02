//! Issuer sector wording into the labels and colour slots the UI uses. Eleven GICS
//! sectors keep their names; colour is seven fixed slots plus two neutrals, so a
//! colour means the same thing on every chart and the palette stays distinguishable.

use crate::AssetClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sector {
    pub label: &'static str,
    /// CSS token suffix: tech, financials, health, industrials, consumer, resources,
    /// communication, other, fixed.
    pub slot: &'static str,
}

pub const UNCLASSIFIED: Sector = Sector { label: "Unclassified", slot: "other" };
pub const FUNDS: Sector = Sector { label: "Funds", slot: "other" };
pub const UNDISCLOSED: Sector = Sector { label: "Undisclosed", slot: "other" };
pub const CASH: Sector = Sector { label: "Cash", slot: "fixed" };
pub const FIXED_INCOME: Sector = Sector { label: "Fixed income", slot: "fixed" };

pub fn canonical(raw: Option<&str>, asset_class: AssetClass) -> Sector {
    match asset_class {
        AssetClass::Cash => return CASH,
        AssetClass::FixedIncome => return FIXED_INCOME,
        _ => {}
    }
    let Some(raw) = raw else { return UNCLASSIFIED };
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s == "-" {
        return UNCLASSIFIED;
    }
    if s.contains("cash") || s.contains("derivative") {
        return CASH;
    }
    if s == "funds" {
        return FUNDS;
    }
    if s.contains("information technology")
        || s == "technology"
        || s.starts_with("tech")
        || s.contains("software")
        || s.contains("semiconductor")
    {
        Sector { label: "Technology", slot: "tech" }
    } else if s.contains("financial") || s.contains("bank") || s.contains("insurance") {
        Sector { label: "Financials", slot: "financials" }
    } else if s.contains("health") || s.contains("pharma") || s.contains("biotech") {
        Sector { label: "Health care", slot: "health" }
    } else if s.contains("industrial") || s.contains("capital goods") || s.contains("transport") {
        Sector { label: "Industrials", slot: "industrials" }
    } else if s.contains("discretionary")
        || s.contains("cyclical")
        || s.contains("retail")
        || s.contains("automo")
    {
        Sector { label: "Consumer discretionary", slot: "consumer" }
    } else if s.contains("staple") || s.contains("defensive") || s.contains("food") || s.contains("beverage")
    {
        Sector { label: "Consumer staples", slot: "consumer" }
    } else if s.contains("energy") || s.contains("oil") || s.contains("gas") {
        Sector { label: "Energy", slot: "resources" }
    } else if s.contains("material") || s.contains("mining") || s.contains("metal") || s.contains("chemical")
    {
        Sector { label: "Materials", slot: "resources" }
    } else if s.contains("communication") || s.contains("telecom") || s.contains("media") {
        Sector { label: "Communication", slot: "communication" }
    } else if s.contains("utilit") {
        Sector { label: "Utilities", slot: "other" }
    } else if s.contains("real estate") || s.contains("reit") || s.contains("property") {
        Sector { label: "Real estate", slot: "other" }
    } else if s.contains("fixed")
        || s.contains("bond")
        || s.contains("treasur")
        || s.contains("government")
        || s.contains("corporate")
    {
        FIXED_INCOME
    } else {
        Sector { label: "Other", slot: "other" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_issuer_wording() {
        assert_eq!(canonical(Some("Information Technology"), AssetClass::Equity).slot, "tech");
        assert_eq!(canonical(Some("Consumer Staples"), AssetClass::Equity).label, "Consumer staples");
        assert_eq!(canonical(Some("Communication"), AssetClass::Equity).slot, "communication");
        assert_eq!(canonical(Some("Cash and/or Derivatives"), AssetClass::Other), CASH);
        assert_eq!(canonical(Some("Financials"), AssetClass::FixedIncome), FIXED_INCOME);
        assert_eq!(canonical(None, AssetClass::Equity), UNCLASSIFIED);
        assert_eq!(canonical(Some("-"), AssetClass::Equity), UNCLASSIFIED);
    }
}
