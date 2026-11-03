//! What the iPhone Shortcut sends for one Apple Pay tap. The Wallet trigger gives
//! merchant, amount, card and name; the Shortcut adds the time. Parsing is lenient
//! because Shortcuts formats amounts with the phone's locale ("$12.34", "CA$1,234.56",
//! "12,34 €") and may write a dictionary as one line or pretty-printed.

use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tap {
    /// From the Shortcut, or derived from time, card, amount and merchant, so a tap
    /// sent twice (a retry, the queue) is stored once.
    pub id: String,
    pub at: DateTime<FixedOffset>,
    pub merchant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Positive for a purchase. At the authorized amount, so before any tip.
    pub amount: Decimal,
    /// ISO code when the amount said so ("US$", "€"); `None` means the card's currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// The Wallet card's name, as Wallet shows it.
    pub card: String,
    /// When this server got it.
    pub received: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TapParse {
    pub taps: Vec<Tap>,
    /// One line per entry that could not be read, saying why.
    pub errors: Vec<String>,
}

/// Read a body or a file: a JSON object, an array of them, or objects one after another
/// (JSON Lines, or pretty-printed dictionaries appended back to back).
pub fn parse_taps(text: &str, received: DateTime<Utc>) -> TapParse {
    let mut out = TapParse::default();
    let mut values = Vec::new();
    let trimmed = text.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return out;
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Array(items)) => values.extend(items),
        Ok(v) => values.push(v),
        Err(_) => {
            let mut stream = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
            loop {
                match stream.next() {
                    Some(Ok(v)) => values.push(v),
                    Some(Err(e)) => {
                        // Skip to the next line and keep going: one bad append must not
                        // hide every tap after it.
                        let at = stream.byte_offset();
                        out.errors.push(format!("not JSON near byte {at}: {e}"));
                        let rest = &trimmed[at.min(trimmed.len())..];
                        let skip = rest.find('\n').map_or(rest.len(), |i| i + 1);
                        let next = &rest[skip..];
                        let more = parse_taps(next, received);
                        out.taps.extend(more.taps);
                        out.errors.extend(more.errors);
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    let mut parsed = Vec::new();
    for v in values {
        match tap_from_value(&v, received) {
            Ok(t) => parsed.push(t),
            Err(e) => out.errors.push(e),
        }
    }
    parsed.extend(std::mem::take(&mut out.taps));
    out.taps = parsed;
    out
}

fn field<'a>(obj: &'a serde_json::Map<String, Value>, names: &[&str]) -> Option<&'a Value> {
    obj.iter()
        .find(|(k, _)| {
            let k = k.to_lowercase().replace([' ', '_', '-'], "");
            names.iter().any(|n| *n == k)
        })
        .map(|(_, v)| v)
        .filter(|v| !v.is_null())
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        other => other.to_string(),
    }
}

fn tap_from_value(v: &Value, received: DateTime<Utc>) -> Result<Tap, String> {
    let Value::Object(obj) = v else { return Err(format!("expected an object, got {v}")) };
    let merchant = field(obj, &["merchant"]).map(text).filter(|s| !s.is_empty());
    let name = field(obj, &["name", "description"]).map(text).filter(|s| !s.is_empty());
    let merchant = merchant.or_else(|| name.clone()).ok_or("tap has no merchant")?;
    let card = field(obj, &["card", "cardorpass", "pass"])
        .map(text)
        .filter(|s| !s.is_empty())
        .ok_or("tap has no card")?;
    let raw_amount = field(obj, &["amount"]).ok_or("tap has no amount")?;
    let (amount, mut currency) = match raw_amount {
        Value::Number(n) => (n.to_string().parse::<Decimal>().map_err(|e| e.to_string())?, None),
        other => parse_amount(&text(other)).ok_or_else(|| format!("can't read amount {other}"))?,
    };
    if let Some(c) = field(obj, &["currency"]).map(text).filter(|s| s.len() == 3) {
        currency = Some(c.to_uppercase());
    }
    let given = field(obj, &["at", "date", "time"]);
    let at = match given {
        Some(v) => parse_time(&text(v)).ok_or_else(|| format!("can't read time {v}"))?,
        None => received.fixed_offset(),
    };
    let id = field(obj, &["id"]).map(text).filter(|s| !s.is_empty()).unwrap_or_else(|| match given {
        Some(_) => {
            let at_ms = at.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string();
            format!("tap-{}", crate::fnv(&[&at_ms, &card, &amount.normalize().to_string(), &merchant]))
        }
        // No time from the phone: the entry itself is the identity, so reading the same
        // file again never makes a second copy.
        None => format!("tap-{}", crate::fnv(&[&v.to_string()])),
    });
    Ok(Tap { id, at, merchant, name, amount, currency, card, received })
}

/// "2026-11-03T16:25:01.123-05:00", "2026-11-03T16:25:01Z", "2026-11-03 16:25:01".
pub fn parse_time(s: &str) -> Option<DateTime<FixedOffset>> {
    let s = s.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Some(t);
    }
    for f in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%d %H:%M:%S%.f%z", "%Y-%m-%dT%H:%M:%S%.f%#z"] {
        if let Ok(t) = DateTime::parse_from_str(s, f) {
            return Some(t);
        }
    }
    for f in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M"] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, f) {
            return Some(t.and_utc().fixed_offset());
        }
    }
    None
}

/// An amount as a person or a phone writes it, and the currency it names if any.
/// `$` alone names nothing: it is the card's own dollars.
pub fn parse_amount(s: &str) -> Option<(Decimal, Option<String>)> {
    let up = s.trim().to_uppercase();
    if up.is_empty() {
        return None;
    }
    let named = [
        ("US$", "USD"),
        ("CA$", "CAD"),
        ("C$", "CAD"),
        ("MX$", "MXN"),
        ("A$", "AUD"),
        ("€", "EUR"),
        ("£", "GBP"),
        ("¥", "JPY"),
    ];
    let currency: Option<String> =
        named.iter().find(|(sym, _)| up.contains(sym)).map(|(_, code)| code.to_string()).or_else(|| {
            // A three-letter code written next to the number ("12.00 USD").
            up.split(|c: char| !c.is_ascii_alphabetic()).find(|w| w.len() == 3).map(str::to_string)
        });
    let negative = up.contains('-') || (up.contains('(') && up.contains(')'));
    let digits: String = up.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    let normalized = normalize_number(&digits)?;
    let mut d: Decimal = normalized.parse().ok()?;
    if negative {
        d = -d;
    }
    Some((d, currency))
}

/// "1,234.56" / "1.234,56" / "12,34" / "1,234" -> "1234.56" / "1234.56" / "12.34" / "1234".
pub(crate) fn normalize_number(s: &str) -> Option<String> {
    if s.is_empty() || !s.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let last_dot = s.rfind('.');
    let last_comma = s.rfind(',');
    let decimal = match (last_dot, last_comma) {
        (Some(d), Some(c)) => Some(d.max(c)),
        (Some(d), None) => {
            // "1.234" with a single dot and three digits after is ambiguous; a dot is a
            // decimal point unless it appears more than once.
            (s.matches('.').count() == 1).then_some(d)
        }
        (None, Some(c)) => {
            let after = s.len() - c - 1;
            (s.matches(',').count() == 1 && after != 3).then_some(c)
        }
        (None, None) => None,
    };
    let mut out = String::new();
    for (i, ch) in s.char_indices() {
        if ch.is_ascii_digit() {
            out.push(ch);
        } else if Some(i) == decimal {
            out.push('.');
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-03T21:30:00Z").unwrap().to_utc()
    }

    #[test]
    fn amounts_as_phones_write_them() {
        assert_eq!(parse_amount("$12.34"), Some((dec!(12.34), None)));
        assert_eq!(parse_amount("CA$1,234.56"), Some((dec!(1234.56), Some("CAD".into()))));
        assert_eq!(parse_amount("US$3.00"), Some((dec!(3.00), Some("USD".into()))));
        assert_eq!(parse_amount("12,34 €"), Some((dec!(12.34), Some("EUR".into()))));
        assert_eq!(parse_amount("1.234,50 €"), Some((dec!(1234.50), Some("EUR".into()))));
        assert_eq!(parse_amount("-$5.00"), Some((dec!(-5.00), None)));
        assert_eq!(parse_amount("1,234"), Some((dec!(1234), None)));
        assert_eq!(parse_amount("£7"), Some((dec!(7), Some("GBP".into()))));
        assert_eq!(parse_amount("free"), None);
    }

    #[test]
    fn one_object_an_array_or_appended_lines() {
        let one = r#"{"merchant":"Blue Bottle","amount":"$7.25","card":"Amex Cobalt","name":"Blue Bottle Coffee","at":"2026-11-03T08:12:45.120-05:00"}"#;
        let p = parse_taps(one, now());
        assert_eq!(p.taps.len(), 1);
        let t = &p.taps[0];
        assert_eq!(t.amount, dec!(7.25));
        assert_eq!(t.currency, None);
        assert_eq!(t.at.to_rfc3339(), "2026-11-03T08:12:45.120-05:00");
        assert!(t.id.starts_with("tap-"));
        // The same tap sent again gets the same id.
        assert_eq!(parse_taps(one, now() + chrono::Duration::hours(3)).taps[0].id, t.id);

        let arr = format!("[{one},{one}]");
        assert_eq!(parse_taps(&arr, now()).taps.len(), 2);

        // Pretty-printed dictionaries appended back to back, with a bad line between.
        let file = format!(
            "{{\n  \"Merchant\": \"Loblaws\",\n  \"Amount\": \"52,18 $\",\n  \"Card or Pass\": \"TD Visa\"\n}}\n{{oops\n{one}\n"
        );
        let p = parse_taps(&file, now());
        assert_eq!(p.taps.len(), 2, "{:?}", p.errors);
        assert_eq!(p.errors.len(), 1);
        assert_eq!(p.taps[0].merchant, "Loblaws");
        assert_eq!(p.taps[0].card, "TD Visa");
        assert_eq!(p.taps[0].amount, dec!(52.18));
    }

    #[test]
    fn missing_fields_are_reported_not_guessed() {
        let p = parse_taps(
            r#"[{"amount":"$1"},{"merchant":"x","card":"y"},{"merchant":"x","card":"y","amount":"?"}]"#,
            now(),
        );
        assert!(p.taps.is_empty());
        assert_eq!(p.errors.len(), 3);
    }

    #[test]
    fn a_tap_without_a_time_keeps_one_id_however_often_it_is_read() {
        let line = r#"{"merchant":"Metro","amount":"$9.99","card":"Visa"}"#;
        let a = parse_taps(line, now());
        let b = parse_taps(line, now() + chrono::Duration::minutes(5));
        assert_eq!(a.taps[0].id, b.taps[0].id);
        assert_eq!(a.taps[0].at, now().fixed_offset());
    }

    #[test]
    fn id_and_currency_from_the_shortcut_win() {
        let p = parse_taps(
            r#"{"id":"abc","merchant":"Shake Shack","amount":"14.20","currency":"usd","card":"Amex","at":"2026-11-03 12:00:00"}"#,
            now(),
        );
        assert_eq!(p.taps[0].id, "abc");
        assert_eq!(p.taps[0].currency.as_deref(), Some("USD"));
    }
}
