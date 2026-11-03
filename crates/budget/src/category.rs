//! Categories. Your rules (in the ledger) come first, then a built-in list of common
//! Canadian merchants, then the sign: money in is income, unless it is a card payment.
//! `transfer` and `income` never count as spending, so paying a card bill from chequing
//! (already counted as purchases on the card) is not counted again.

use ledger::CategoryRule;
use rust_decimal::Decimal;

/// (id, label) in display order.
pub const CATEGORIES: [(&str, &str); 13] = [
    ("groceries", "Groceries"),
    ("dining", "Dining"),
    ("coffee", "Coffee"),
    ("transport", "Transport"),
    ("shopping", "Shopping"),
    ("bills", "Bills"),
    ("entertainment", "Entertainment"),
    ("health", "Health"),
    ("travel", "Travel"),
    ("fees", "Fees and interest"),
    ("other", "Other"),
    ("income", "Income"),
    ("transfer", "Transfer"),
];

pub fn label(category: &str) -> String {
    CATEGORIES.iter().find(|(id, _)| *id == category).map(|(_, l)| l.to_string()).unwrap_or_else(|| {
        let mut c = category.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    })
}

pub fn counts_as_spending(category: &str) -> bool {
    !matches!(category, "income" | "transfer")
}

/// The category where a restaurant tip can make the statement amount higher than the tap.
pub const TIPPED: [&str; 2] = ["dining", "coffee"];

/// Built-in rules, checked in order as whole words, so "BAR" does not match "BARBER".
/// More specific entries come before the general ones ("UBER EATS" before "UBER").
const BUILT_IN: &[(&str, &[&str])] = &[
    ("other", &["E TRANSFER", "E TFR", "INTERAC E", "SEND E"]),
    (
        "transfer",
        &[
            "PAYMENT THANK YOU",
            "PAYMENT RECEIVED",
            "THANK YOU PAYMENT",
            "PAIEMENT MERCI",
            "BILL PYMT",
            "CARD PAYMENT",
            "VISA PAYMENT",
            "MASTERCARD PAYMENT",
            "AMEX PAYMENT",
            "ONLINE TRANSFER",
            "TFR TO",
            "TFR FROM",
            "TRANSFER TO",
            "TRANSFER FROM",
            "WS INVESTMENTS",
            "WEALTHSIMPLE",
            "QUESTRADE",
        ],
    ),
    (
        "income",
        &[
            "PAYROLL",
            "PAY DEP",
            "DIRECT DEP",
            "DIRECT DEPOSIT",
            "CANADA FED",
            "CANADA RIT",
            "GST",
            "CCB",
            "EI CANADA",
            "INTEREST PAID",
            "INTEREST EARNED",
        ],
    ),
    (
        "fees",
        &[
            "INTEREST CHARGE",
            "PURCHASE INTEREST",
            "ANNUAL FEE",
            "MONTHLY FEE",
            "SERVICE CHARGE",
            "NSF",
            "OVERDRAFT",
            "FOREIGN TRANSACTION FEE",
        ],
    ),
    (
        "dining",
        &[
            "UBER EATS",
            "UBEREATS",
            "DOORDASH",
            "SKIP THE DISHES",
            "SKIPTHEDISHES",
            "RESTAURANT",
            "MCDONALD",
            "MCDONALDS",
            "A&W",
            "SUBWAY",
            "CHIPOTLE",
            "PIZZA",
            "PIZZERIA",
            "SUSHI",
            "THAI",
            "PHO",
            "RAMEN",
            "BURGER",
            "SHAWARMA",
            "BAR",
            "PUB",
            "GRILL",
            "KITCHEN",
            "KITCH",
            "BISTRO",
            "TAVERN",
            "DINER",
            "TACO",
            "WENDY",
            "WENDYS",
            "HARVEY",
            "HARVEYS",
            "POPEYES",
            "KFC",
            "NANDO",
            "OSMOW",
            "FRESHII",
            "MARY BROWN",
            "BOSTON PIZZA",
            "EARLS",
            "CACTUS CLUB",
            "THE KEG",
            "JOEY",
        ],
    ),
    (
        "coffee",
        &[
            "STARBUCKS",
            "TIM HORTONS",
            "TIMS",
            "BLUE BOTTLE",
            "SECOND CUP",
            "BALZAC",
            "PILOT COFFEE",
            "COFFEE",
            "CAFE",
            "ESPRESSO",
            "BALZACS",
            "DAVIDSTEA",
        ],
    ),
    (
        "groceries",
        &[
            "LOBLAWS",
            "NO FRILLS",
            "NOFRILLS",
            "METRO",
            "SOBEYS",
            "FOOD BASICS",
            "FRESHCO",
            "FARM BOY",
            "WHOLE FOODS",
            "T&T",
            "LONGOS",
            "LONGO",
            "COSTCO",
            "SAFEWAY",
            "SAVE ON FOODS",
            "IGA",
            "PROVIGO",
            "MAXI",
            "SUPERSTORE",
            "ZEHRS",
            "FORTINOS",
            "INSTACART",
            "VOILA",
            "FOODLAND",
            "COOP",
            "GROCERY",
            "GROCERIES",
            "SUPERMARKET",
            "VALU MART",
            "YOUR INDEPENDENT",
        ],
    ),
    (
        "transport",
        &[
            "PRESTO",
            "TTC",
            "UBER",
            "LYFT",
            "GO TRANSIT",
            "STM",
            "OC TRANSPO",
            "TRANSLINK",
            "COMPASS",
            "ESSO",
            "PETRO CANADA",
            "PETROCAN",
            "SHELL",
            "PIONEER",
            "ULTRAMAR",
            "HUSKY",
            "CIRCLE K",
            "MOBIL",
            "IMPARK",
            "GREEN P",
            "PARKING",
            "407 ETR",
            "BIXI",
            "COMMUNAUTO",
            "ZIPCAR",
            "EVO",
        ],
    ),
    (
        "travel",
        &[
            "AIR CANADA",
            "WESTJET",
            "PORTER",
            "FLAIR",
            "AIRBNB",
            "EXPEDIA",
            "BOOKING COM",
            "HOTEL",
            "MARRIOTT",
            "HILTON",
            "HYATT",
            "VIA RAIL",
            "PEARSON",
        ],
    ),
    (
        "bills",
        &[
            "ROGERS",
            "BELL CANADA",
            "BELL MOBILITY",
            "TELUS",
            "FIDO",
            "KOODO",
            "VIRGIN PLUS",
            "FREEDOM MOBILE",
            "PUBLIC MOBILE",
            "HYDRO",
            "ENBRIDGE",
            "ALECTRA",
            "FORTISBC",
            "INSURANCE",
            "INTACT",
            "TD INSURANCE",
            "BELAIR",
            "RENT",
            "CONDO FEE",
        ],
    ),
    (
        "entertainment",
        &[
            "NETFLIX",
            "SPOTIFY",
            "DISNEY",
            "CRAVE",
            "CINEPLEX",
            "STEAM",
            "PLAYSTATION",
            "XBOX",
            "NINTENDO",
            "TICKETMASTER",
            "YOUTUBE",
            "APPLE COM BILL",
            "AUDIBLE",
            "PRIME VIDEO",
        ],
    ),
    (
        "health",
        &[
            "SHOPPERS DRUG MART",
            "SHOPPERS",
            "REXALL",
            "PHARMACY",
            "PHARMA",
            "DENTAL",
            "DENTIST",
            "CLINIC",
            "LONDON DRUGS",
            "JEAN COUTU",
            "GOODLIFE",
            "GYM",
            "PHYSIO",
            "OPTOMETR",
        ],
    ),
    (
        "shopping",
        &[
            "AMAZON",
            "AMZN",
            "CANADIAN TIRE",
            "BEST BUY",
            "IKEA",
            "WINNERS",
            "HOMESENSE",
            "HOME DEPOT",
            "WALMART",
            "DOLLARAMA",
            "INDIGO",
            "APPLE STORE",
            "UNIQLO",
            "ZARA",
            "H&M",
            "SPORT CHEK",
            "MEC",
            "HUDSON S BAY",
            "LULULEMON",
            "SIMONS",
            "STAPLES",
            "LCBO",
            "SAQ",
            "BEER STORE",
            "SEPHORA",
        ],
    ),
];

/// Upper-case words, punctuation as spaces ("SQ *BLUE BOTTLE #12" -> "SQ BLUE BOTTLE 12").
pub fn words(s: &str) -> Vec<String> {
    s.to_uppercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '&'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn contains_words(hay: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// The category for a transaction described by `text` (merchant and/or statement text).
/// `amount` is positive out; `liability` says the account is a card or loan, where money
/// in is a payment or a refund, not income.
pub fn categorize(
    text: &str,
    account: Option<&str>,
    amount: Decimal,
    liability: bool,
    rules: &[CategoryRule],
) -> String {
    let up = text.to_uppercase();
    for r in rules {
        if r.account.as_deref().is_some_and(|a| Some(a) != account) {
            continue;
        }
        if !r.contains.is_empty() && up.contains(&r.contains.to_uppercase()) {
            return r.category.clone();
        }
    }
    let hay = words(text);
    for (category, patterns) in BUILT_IN {
        if patterns.iter().any(|p| contains_words(&hay, &words(p))) {
            // Income words on a card are a refund or credit; keep looking.
            if *category == "income" && liability {
                continue;
            }
            return (*category).to_string();
        }
    }
    if amount.is_sign_negative() {
        // Money in with no rule: a payment to a card, or income to a bank account.
        return if liability { "transfer".into() } else { "income".into() };
    }
    "other".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn cat(text: &str) -> String {
        categorize(text, None, dec!(10), false, &[])
    }

    #[test]
    fn built_in_merchants() {
        assert_eq!(cat("SQ *BLUE BOTTLE TORONTO ON"), "coffee");
        assert_eq!(cat("UBER CANADA/UBEREATS TORONTO ON"), "dining");
        assert_eq!(cat("UBER CANADA/UBERTRIP"), "transport");
        assert_eq!(cat("LOBLAWS #1012"), "groceries");
        assert_eq!(cat("PETRO-CANADA 1234"), "transport");
        assert_eq!(cat("PAI NORTHERN THAI KITCH TORONTO"), "dining");
        assert_eq!(cat("BARBERSHOP 22"), "other", "BAR is a word, not a prefix");
        assert_eq!(cat("AMEX BILL PYMT"), "transfer");
        assert_eq!(cat("SEND E-TFR ***Xq7"), "other", "an e-transfer out is spending you categorize");
    }

    #[test]
    fn money_in_is_income_on_a_bank_and_a_payment_on_a_card() {
        assert_eq!(categorize("PAYROLL DEP ACME", None, dec!(-2850), false, &[]), "income");
        assert_eq!(categorize("SOMETHING", None, dec!(-50), false, &[]), "income");
        assert_eq!(categorize("SOMETHING", None, dec!(-50), true, &[]), "transfer");
        assert_eq!(categorize("PAYMENT RECEIVED - THANK YOU", None, dec!(-640), true, &[]), "transfer");
        // A refund at a known merchant stays in that merchant's category (negative spending).
        assert_eq!(categorize("LOBLAWS REFUND", None, dec!(-12), true, &[]), "groceries");
    }

    #[test]
    fn your_rules_come_first_and_can_be_per_account() {
        let rules = vec![
            CategoryRule {
                contains: "costco".into(),
                category: "shopping".into(),
                account: Some("visa".into()),
            },
            CategoryRule { contains: "ACME".into(), category: "income".into(), account: None },
        ];
        assert_eq!(categorize("COSTCO WHOLESALE", Some("visa"), dec!(200), true, &rules), "shopping");
        assert_eq!(categorize("COSTCO WHOLESALE", Some("amex"), dec!(200), true, &rules), "groceries");
        assert_eq!(categorize("ACME", None, dec!(-1), false, &rules), "income");
    }

    #[test]
    fn labels() {
        assert_eq!(label("dining"), "Dining");
        assert_eq!(label("pets"), "Pets");
        assert!(!counts_as_spending("transfer") && counts_as_spending("dining"));
    }
}
