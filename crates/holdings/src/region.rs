//! Country (as issuers spell it) to one of six regions. Unknown countries land in
//! "Other" rather than being dropped, so the bars still sum to the total.

pub const NORTH_AMERICA: &str = "North America";
pub const EUROPE: &str = "Europe";
pub const ASIA_PACIFIC: &str = "Asia-Pacific";
pub const LATIN_AMERICA: &str = "Latin America";
pub const MEA: &str = "Middle East & Africa";
pub const OTHER: &str = "Other";

pub fn region(country: Option<&str>) -> &'static str {
    let Some(c) = country else { return OTHER };
    let c = c.trim().to_ascii_lowercase();
    if c.is_empty() || c == "-" {
        return OTHER;
    }
    const NA: &[&str] =
        &["united states", "usa", "us", "u.s.", "canada", "bermuda", "cayman", "puerto rico", "bahamas"];
    const EU: &[&str] = &[
        "united kingdom",
        "uk",
        "britain",
        "france",
        "germany",
        "switzerland",
        "netherlands",
        "sweden",
        "denmark",
        "spain",
        "italy",
        "belgium",
        "finland",
        "norway",
        "ireland",
        "austria",
        "portugal",
        "poland",
        "greece",
        "hungary",
        "czech",
        "luxembourg",
        "jersey",
        "guernsey",
        "isle of man",
        "iceland",
        "liechtenstein",
        "monaco",
        "cyprus",
        "malta",
        "romania",
        "turkey",
        "türkiye",
        "russia",
        "estonia",
        "latvia",
        "lithuania",
        "slovenia",
        "slovakia",
        "croatia",
        "faroe",
    ];
    const AP: &[&str] = &[
        "japan",
        "china",
        "hong kong",
        "taiwan",
        "korea",
        "india",
        "australia",
        "singapore",
        "indonesia",
        "thailand",
        "malaysia",
        "philippines",
        "new zealand",
        "vietnam",
        "macau",
        "macao",
        "pakistan",
        "sri lanka",
        "bangladesh",
        "kazakhstan",
        "mongolia",
        "cambodia",
    ];
    const LA: &[&str] = &[
        "brazil",
        "mexico",
        "chile",
        "colombia",
        "peru",
        "argentina",
        "panama",
        "uruguay",
        "costa rica",
        "venezuela",
    ];
    const ME: &[&str] = &[
        "saudi",
        "south africa",
        "united arab emirates",
        "uae",
        "qatar",
        "kuwait",
        "israel",
        "egypt",
        "nigeria",
        "kenya",
        "morocco",
        "bahrain",
        "oman",
        "jordan",
        "mauritius",
        "ghana",
        "zambia",
        "africa",
    ];
    let hit = |list: &[&str]| list.iter().any(|k| c == *k || c.contains(k));
    if hit(NA) {
        NORTH_AMERICA
    } else if hit(EU) {
        EUROPE
    } else if hit(AP) {
        ASIA_PACIFIC
    } else if hit(LA) {
        LATIN_AMERICA
    } else if hit(ME) {
        MEA
    } else {
        OTHER
    }
}

/// Listing country from a ledger symbol's suffix, for as-held views and for a stock
/// with no metadata: `.TO`/`.V`/`.NE`/`.CN` Canada, `.L` United Kingdom, none United States.
pub fn country_from_symbol(symbol: &str) -> Option<&'static str> {
    let suffix = symbol.rsplit_once('.').map(|(_, s)| s.to_ascii_uppercase());
    Some(match suffix.as_deref() {
        None => "United States",
        Some("TO" | "V" | "NE" | "CN" | "TSX") => "Canada",
        Some("L") => "United Kingdom",
        Some("DE" | "F") => "Germany",
        Some("PA") => "France",
        Some("SW") => "Switzerland",
        Some("AS") => "Netherlands",
        Some("T") => "Japan",
        Some("HK") => "Hong Kong",
        Some("AX") => "Australia",
        Some("TW") => "Taiwan",
        Some("KS") => "Korea",
        Some("NS" | "BO") => "India",
        Some("SA") => "Brazil",
        Some("MX") => "Mexico",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_countries() {
        assert_eq!(region(Some("United States")), NORTH_AMERICA);
        assert_eq!(region(Some("Korea (South)")), ASIA_PACIFIC);
        assert_eq!(region(Some("Saudi Arabia")), MEA);
        assert_eq!(region(Some("Atlantis")), OTHER);
        assert_eq!(region(None), OTHER);
        assert_eq!(country_from_symbol("XEQT.TO"), Some("Canada"));
        assert_eq!(country_from_symbol("VTI"), Some("United States"));
    }
}
