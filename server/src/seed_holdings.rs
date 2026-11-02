//! Sample fund compositions for `worthwatch seed`: what `holdings/` looks like after a
//! refresh. Names are real constituents; weights are rounded and the tails are
//! shortened, so every number here is illustrative. Deterministic.

use chrono::NaiveDate;
use holdings::{AssetClass, Constituent, FundHoldings};
use rust_decimal::Decimal;

const US: &str = "United States";
const CA: &str = "Canada";

type Row = (&'static str, &'static str, &'static str, &'static str, u32);

fn w(bp: u32) -> Decimal {
    Decimal::new(i64::from(bp), 2)
}

/// Rows as (ticker, name, sector, country, weight in hundredths of a percent). The tail
/// is scaled so the rows sum to 100.00 exactly.
fn fund(
    symbol: &str,
    name: &str,
    as_of: NaiveDate,
    rows: &[Row],
    exchange: &str,
    currency: &str,
) -> FundHoldings {
    let listed: u32 = rows.iter().map(|r| r.4).sum();
    let mut out: Vec<Constituent> = rows
        .iter()
        .map(|(t, n, s, c, bp)| Constituent {
            ticker: Some((*t).into()),
            name: (*n).into(),
            weight: w(*bp),
            sector: (!s.is_empty()).then(|| (*s).into()),
            country: Some((*c).into()),
            asset_class: if *s == "Cash and/or Derivatives" { AssetClass::Cash } else { AssetClass::Equity },
            currency: Some(currency.into()),
            exchange: (!exchange.is_empty()).then(|| exchange.into()),
        })
        .collect();
    // Scale everything so the file sums to 100.00, the way an issuer's full file does.
    let scale = Decimal::new(10000, 0) / Decimal::from(listed);
    for c in &mut out {
        c.weight = (c.weight * scale).round_dp(2);
    }
    let sum: Decimal = out.iter().map(|c| c.weight).sum();
    if let Some(first) = out.first_mut() {
        first.weight += Decimal::ONE_HUNDRED - sum;
    }
    FundHoldings { symbol: symbol.into(), name: Some(name.into()), as_of, source: "seed".into(), rows: out }
}

const SP500: &[Row] = &[
    ("NVDA", "NVIDIA CORP", "Information Technology", US, 791),
    ("AAPL", "APPLE INC", "Information Technology", US, 684),
    ("MSFT", "MICROSOFT CORP", "Information Technology", US, 642),
    ("AMZN", "AMAZON COM INC", "Consumer Discretionary", US, 391),
    ("AVGO", "BROADCOM INC", "Information Technology", US, 259),
    ("META", "META PLATFORMS INC CLASS A", "Communication", US, 252),
    ("GOOGL", "ALPHABET INC CLASS A", "Communication", US, 221),
    ("GOOG", "ALPHABET INC CLASS C", "Communication", US, 180),
    ("TSLA", "TESLA INC", "Consumer Discretionary", US, 182),
    ("BRK.B", "BERKSHIRE HATHAWAY INC CLASS B", "Financials", US, 161),
    ("JPM", "JPMORGAN CHASE & CO", "Financials", US, 144),
    ("LLY", "ELI LILLY", "Health Care", US, 130),
    ("V", "VISA INC CLASS A", "Financials", US, 115),
    ("XOM", "EXXON MOBIL CORP", "Energy", US, 96),
    ("NFLX", "NETFLIX INC", "Communication", US, 95),
    ("MA", "MASTERCARD INC CLASS A", "Financials", US, 90),
    ("WMT", "WALMART INC", "Consumer Staples", US, 86),
    ("COST", "COSTCO WHOLESALE CORP", "Consumer Staples", US, 82),
    ("JNJ", "JOHNSON & JOHNSON", "Health Care", US, 74),
    ("ORCL", "ORACLE CORP", "Information Technology", US, 72),
    ("PG", "PROCTER & GAMBLE", "Consumer Staples", US, 70),
    ("HD", "HOME DEPOT INC", "Consumer Discretionary", US, 68),
    ("ABBV", "ABBVIE INC", "Health Care", US, 64),
    ("BAC", "BANK OF AMERICA CORP", "Financials", US, 60),
    ("PLTR", "PALANTIR TECHNOLOGIES INC CLASS A", "Information Technology", US, 58),
    ("KO", "COCA-COLA", "Consumer Staples", US, 52),
    ("CVX", "CHEVRON CORP", "Energy", US, 50),
    ("UNH", "UNITEDHEALTH GROUP INC", "Health Care", US, 48),
    ("CRM", "SALESFORCE INC", "Information Technology", US, 46),
    ("CSCO", "CISCO SYSTEMS INC", "Information Technology", US, 45),
    ("WFC", "WELLS FARGO", "Financials", US, 44),
    ("AMD", "ADVANCED MICRO DEVICES INC", "Information Technology", US, 43),
    ("PM", "PHILIP MORRIS INTERNATIONAL INC", "Consumer Staples", US, 42),
    ("LIN", "LINDE PLC", "Materials", US, 41),
    ("MRK", "MERCK & CO INC", "Health Care", US, 40),
    ("GE", "GE AEROSPACE", "Industrials", US, 40),
    ("CAT", "CATERPILLAR INC", "Industrials", US, 38),
    ("RTX", "RTX CORP", "Industrials", US, 36),
    ("UBER", "UBER TECHNOLOGIES INC", "Industrials", US, 34),
    ("HON", "HONEYWELL INTERNATIONAL INC", "Industrials", US, 32),
    ("NEE", "NEXTERA ENERGY INC", "Utilities", US, 30),
    ("PLD", "PROLOGIS REIT INC", "Real Estate", US, 26),
    ("T", "AT&T INC", "Communication", US, 36),
    ("DIS", "WALT DISNEY", "Communication", US, 34),
    ("INTU", "INTUIT INC", "Information Technology", US, 33),
    ("GS", "GOLDMAN SACHS GROUP INC", "Financials", US, 36),
    ("MS", "MORGAN STANLEY", "Financials", US, 34),
    ("AXP", "AMERICAN EXPRESS", "Financials", US, 32),
    ("TMO", "THERMO FISHER SCIENTIFIC INC", "Health Care", US, 30),
    ("ABT", "ABBOTT LABORATORIES", "Health Care", US, 30),
    ("ISRG", "INTUITIVE SURGICAL INC", "Health Care", US, 28),
    ("NOW", "SERVICENOW INC", "Information Technology", US, 28),
    ("ACN", "ACCENTURE PLC CLASS A", "Information Technology", US, 27),
    ("IBM", "INTERNATIONAL BUSINESS MACHINES CO", "Information Technology", US, 27),
    ("PEP", "PEPSICO INC", "Consumer Staples", US, 26),
    ("MCD", "MCDONALDS CORP", "Consumer Discretionary", US, 28),
    ("BKNG", "BOOKING HOLDINGS INC", "Consumer Discretionary", US, 26),
    ("QCOM", "QUALCOMM INC", "Information Technology", US, 24),
    ("TXN", "TEXAS INSTRUMENT INC", "Information Technology", US, 24),
    ("AMGN", "AMGEN INC", "Health Care", US, 22),
    ("DHR", "DANAHER CORP", "Health Care", US, 20),
    ("SPGI", "S&P GLOBAL INC", "Financials", US, 22),
    ("BLK", "BLACKROCK INC", "Financials", US, 20),
    ("ADBE", "ADOBE INC", "Information Technology", US, 22),
    ("LOW", "LOWES COMPANIES INC", "Consumer Discretionary", US, 20),
    ("UNP", "UNION PACIFIC CORP", "Industrials", US, 20),
    ("COP", "CONOCOPHILLIPS", "Energy", US, 18),
    ("BA", "BOEING", "Industrials", US, 20),
    ("ETN", "EATON PLC", "Industrials", US, 20),
    ("SO", "SOUTHERN", "Utilities", US, 14),
    ("DUK", "DUKE ENERGY CORP", "Utilities", US, 12),
    ("AMT", "AMERICAN TOWER REIT CORP", "Real Estate", US, 12),
    ("SHW", "SHERWIN WILLIAMS", "Materials", US, 12),
    ("FCX", "FREEPORT MCMORAN INC", "Materials", US, 10),
    ("USD", "USD CASH", "Cash and/or Derivatives", US, 20),
];

/// VTI/ITOT: the S&P names plus mid and small caps.
const US_TOTAL: &[Row] = &[
    ("NVDA", "NVIDIA CORP", "Information Technology", US, 690),
    ("AAPL", "APPLE INC", "Information Technology", US, 621),
    ("MSFT", "MICROSOFT CORP", "Information Technology", US, 556),
    ("AMZN", "AMAZON COM INC", "Consumer Discretionary", US, 340),
    ("AVGO", "BROADCOM INC", "Information Technology", US, 224),
    ("META", "META PLATFORMS INC CLASS A", "Communication", US, 218),
    ("GOOGL", "ALPHABET INC CLASS A", "Communication", US, 191),
    ("GOOG", "ALPHABET INC CLASS C", "Communication", US, 156),
    ("TSLA", "TESLA INC", "Consumer Discretionary", US, 158),
    ("BRK.B", "BERKSHIRE HATHAWAY INC CLASS B", "Financials", US, 140),
    ("JPM", "JPMORGAN CHASE & CO", "Financials", US, 125),
    ("LLY", "ELI LILLY", "Health Care", US, 113),
    ("V", "VISA INC CLASS A", "Financials", US, 100),
    ("XOM", "EXXON MOBIL CORP", "Energy", US, 83),
    ("NFLX", "NETFLIX INC", "Communication", US, 82),
    ("MA", "MASTERCARD INC CLASS A", "Financials", US, 78),
    ("WMT", "WALMART INC", "Consumer Staples", US, 75),
    ("COST", "COSTCO WHOLESALE CORP", "Consumer Staples", US, 71),
    ("JNJ", "JOHNSON & JOHNSON", "Health Care", US, 64),
    ("ORCL", "ORACLE CORP", "Information Technology", US, 62),
    ("PG", "PROCTER & GAMBLE", "Consumer Staples", US, 61),
    ("HD", "HOME DEPOT INC", "Consumer Discretionary", US, 59),
    ("ABBV", "ABBVIE INC", "Health Care", US, 56),
    ("BAC", "BANK OF AMERICA CORP", "Financials", US, 52),
    ("PLTR", "PALANTIR TECHNOLOGIES INC CLASS A", "Information Technology", US, 50),
    ("KO", "COCA-COLA", "Consumer Staples", US, 45),
    ("CVX", "CHEVRON CORP", "Energy", US, 44),
    ("UNH", "UNITEDHEALTH GROUP INC", "Health Care", US, 42),
    ("CRM", "SALESFORCE INC", "Information Technology", US, 40),
    ("CSCO", "CISCO SYSTEMS INC", "Information Technology", US, 39),
    ("WFC", "WELLS FARGO", "Financials", US, 38),
    ("AMD", "ADVANCED MICRO DEVICES INC", "Information Technology", US, 37),
    ("PM", "PHILIP MORRIS INTERNATIONAL INC", "Consumer Staples", US, 36),
    ("LIN", "LINDE PLC", "Materials", US, 36),
    ("MRK", "MERCK & CO INC", "Health Care", US, 35),
    ("GE", "GE AEROSPACE", "Industrials", US, 35),
    ("CAT", "CATERPILLAR INC", "Industrials", US, 33),
    ("RTX", "RTX CORP", "Industrials", US, 31),
    ("UBER", "UBER TECHNOLOGIES INC", "Industrials", US, 30),
    ("HON", "HONEYWELL INTERNATIONAL INC", "Industrials", US, 28),
    ("NEE", "NEXTERA ENERGY INC", "Utilities", US, 26),
    ("PLD", "PROLOGIS REIT INC", "Real Estate", US, 23),
    ("T", "AT&T INC", "Communication", US, 31),
    ("DIS", "WALT DISNEY", "Communication", US, 30),
    ("GS", "GOLDMAN SACHS GROUP INC", "Financials", US, 31),
    ("MS", "MORGAN STANLEY", "Financials", US, 30),
    ("TMO", "THERMO FISHER SCIENTIFIC INC", "Health Care", US, 26),
    ("ABT", "ABBOTT LABORATORIES", "Health Care", US, 26),
    ("ISRG", "INTUITIVE SURGICAL INC", "Health Care", US, 24),
    ("NOW", "SERVICENOW INC", "Information Technology", US, 24),
    ("PEP", "PEPSICO INC", "Consumer Staples", US, 23),
    ("MCD", "MCDONALDS CORP", "Consumer Discretionary", US, 24),
    ("QCOM", "QUALCOMM INC", "Information Technology", US, 21),
    ("AMGN", "AMGEN INC", "Health Care", US, 19),
    ("UNP", "UNION PACIFIC CORP", "Industrials", US, 18),
    ("COP", "CONOCOPHILLIPS", "Energy", US, 16),
    ("BA", "BOEING", "Industrials", US, 18),
    ("SO", "SOUTHERN", "Utilities", US, 12),
    ("AMT", "AMERICAN TOWER REIT CORP", "Real Estate", US, 10),
    ("FCX", "FREEPORT MCMORAN INC", "Materials", US, 9),
    ("APP", "APPLOVIN CORP CLASS A", "Information Technology", US, 22),
    ("CRWD", "CROWDSTRIKE HOLDINGS INC CLASS A", "Information Technology", US, 18),
    ("SNOW", "SNOWFLAKE INC CLASS A", "Information Technology", US, 12),
    ("SPOT", "SPOTIFY TECHNOLOGY SA", "Communication", US, 10),
    ("ARES", "ARES MANAGEMENT CORP CLASS A", "Financials", US, 8),
    ("DKNG", "DRAFTKINGS INC CLASS A", "Consumer Discretionary", US, 6),
    ("RIVN", "RIVIAN AUTOMOTIVE INC CLASS A", "Consumer Discretionary", US, 5),
    ("SOFI", "SOFI TECHNOLOGIES INC", "Financials", US, 6),
    ("CVNA", "CARVANA CLASS A", "Consumer Discretionary", US, 8),
    ("MSTR", "STRATEGY INC CLASS A", "Information Technology", US, 12),
    ("VRT", "VERTIV HOLDINGS CLASS A", "Industrials", US, 10),
    ("RBLX", "ROBLOX CORP CLASS A", "Communication", US, 8),
    ("DUOL", "DUOLINGO INC CLASS A", "Information Technology", US, 4),
    ("USD", "USD CASH", "Cash and/or Derivatives", US, 15),
];

const CANADA: &[Row] = &[
    ("RY", "ROYAL BANK OF CANADA", "Financials", CA, 752),
    ("SHOP", "SHOPIFY SUBORDINATE VOTING INC CLASS A", "Information Technology", CA, 617),
    ("TD", "TORONTO DOMINION", "Financials", CA, 446),
    ("ENB", "ENBRIDGE INC", "Energy", CA, 372),
    ("BN", "BROOKFIELD CORP CLASS A", "Financials", CA, 317),
    ("CP", "CANADIAN PACIFIC KANSAS CITY LTD", "Industrials", CA, 287),
    ("BMO", "BANK OF MONTREAL", "Financials", CA, 284),
    ("CNQ", "CANADIAN NATURAL RESOURCES LTD", "Energy", CA, 269),
    ("CNR", "CANADIAN NATIONAL RAILWAY", "Industrials", CA, 260),
    ("BNS", "BANK OF NOVA SCOTIA", "Financials", CA, 250),
    ("CSU", "CONSTELLATION SOFTWARE INC", "Information Technology", CA, 235),
    ("ABX", "BARRICK MINING CORP", "Materials", CA, 226),
    ("AEM", "AGNICO EAGLE MINES LTD", "Materials", CA, 220),
    ("CM", "CANADIAN IMPERIAL BANK OF COMMERCE", "Financials", CA, 211),
    ("WCN", "WASTE CONNECTIONS INC", "Industrials", CA, 183),
    ("TRP", "TC ENERGY CORP", "Energy", CA, 174),
    ("MFC", "MANULIFE FINANCIAL CORP", "Financials", CA, 171),
    ("SU", "SUNCOR ENERGY INC", "Energy", CA, 168),
    ("ATD", "ALIMENTATION COUCHE-TARD INC", "Consumer Staples", CA, 153),
    ("WPM", "WHEATON PRECIOUS METALS CORP", "Materials", CA, 147),
    ("FTS", "FORTIS INC", "Utilities", CA, 113),
    ("SLF", "SUN LIFE FINANCIAL INC", "Financials", CA, 130),
    ("IFC", "INTACT FINANCIAL CORP", "Financials", CA, 120),
    ("BAM", "BROOKFIELD ASSET MANAGEMENT LTD CLASS A", "Financials", CA, 100),
    ("NTR", "NUTRIEN LTD", "Materials", CA, 95),
    ("L", "LOBLAW COMPANIES LTD", "Consumer Staples", CA, 90),
    ("DOL", "DOLLARAMA INC", "Consumer Discretionary", CA, 95),
    ("GIB.A", "CGI INC CLASS A", "Information Technology", CA, 80),
    ("FNV", "FRANCO NEVADA CORP", "Materials", CA, 85),
    ("CCO", "CAMECO CORP", "Energy", CA, 90),
    ("IMO", "IMPERIAL OIL LTD", "Energy", CA, 70),
    ("PPL", "PEMBINA PIPELINE CORP", "Energy", CA, 65),
    ("POW", "POWER CORPORATION OF CANADA", "Financials", CA, 60),
    ("NA", "NATIONAL BANK OF CANADA", "Financials", CA, 110),
    ("T", "TELUS CORP", "Communication", CA, 73),
    ("BCE", "BCE INC", "Communication", CA, 70),
    ("RCI.B", "ROGERS COMMUNICATIONS INC CLASS B", "Communication", CA, 45),
    ("CAR.UN", "CANADIAN APARTMENT PROPERTIES REIT", "Real Estate", CA, 37),
    ("WSP", "WSP GLOBAL INC", "Industrials", CA, 80),
    ("TRI", "THOMSON REUTERS CORP", "Industrials", CA, 80),
    ("MG", "MAGNA INTERNATIONAL INC", "Consumer Discretionary", CA, 40),
    ("QSR", "RESTAURANT BRANDS INTERNATIONAL INC", "Consumer Discretionary", CA, 55),
    ("EMA", "EMERA INC", "Utilities", CA, 40),
    ("H", "HYDRO ONE LTD", "Utilities", CA, 45),
    ("TOU", "TOURMALINE OIL CORP", "Energy", CA, 55),
    ("ARX", "ARC RESOURCES LTD", "Energy", CA, 40),
    ("TECK.B", "TECK RESOURCES LTD CLASS B", "Materials", CA, 60),
    ("FM", "FIRST QUANTUM MINERALS LTD", "Materials", CA, 45),
    ("CAD", "CAD CASH", "Cash and/or Derivatives", CA, 8),
];

const DEVELOPED: &[Row] = &[
    ("ASML", "ASML HOLDING NV", "Information Technology", "Netherlands", 210),
    ("SAP", "SAP", "Information Technology", "Germany", 165),
    ("NOVO B", "NOVO NORDISK CLASS B", "Health Care", "Denmark", 150),
    ("NESN", "NESTLE SA", "Consumer Staples", "Switzerland", 145),
    ("7203", "TOYOTA MOTOR CORP", "Consumer Discretionary", "Japan", 130),
    ("AZN", "ASTRAZENECA PLC", "Health Care", "United Kingdom", 125),
    ("ROG", "ROCHE HOLDING PAR AG", "Health Care", "Switzerland", 120),
    ("NOVN", "NOVARTIS AG", "Health Care", "Switzerland", 115),
    ("SHEL", "SHELL PLC", "Energy", "United Kingdom", 110),
    ("HSBA", "HSBC HOLDINGS PLC", "Financials", "United Kingdom", 105),
    ("MC", "LVMH", "Consumer Discretionary", "France", 95),
    ("SIE", "SIEMENS N AG", "Industrials", "Germany", 95),
    ("8306", "MITSUBISHI UFJ FINANCIAL GROUP INC", "Financials", "Japan", 90),
    ("6758", "SONY GROUP CORP", "Consumer Discretionary", "Japan", 85),
    ("CBA", "COMMONWEALTH BANK OF AUSTRALIA", "Financials", "Australia", 85),
    ("ULVR", "UNILEVER PLC", "Consumer Staples", "United Kingdom", 75),
    ("ALV", "ALLIANZ", "Financials", "Germany", 70),
    ("TTE", "TOTALENERGIES", "Energy", "France", 70),
    ("SAN", "SANOFI SA", "Health Care", "France", 60),
    ("BHP", "BHP GROUP LTD", "Materials", "Australia", 70),
    ("8035", "TOKYO ELECTRON LTD", "Information Technology", "Japan", 60),
    ("6861", "KEYENCE CORP", "Information Technology", "Japan", 55),
    ("UBSG", "UBS GROUP AG", "Financials", "Switzerland", 55),
    ("AIR", "AIRBUS SE", "Industrials", "France", 55),
    ("RMS", "HERMES INTERNATIONAL SCA", "Consumer Discretionary", "France", 50),
    ("OR", "L OREAL SA", "Consumer Staples", "France", 45),
    ("SU", "SCHNEIDER ELECTRIC", "Industrials", "France", 55),
    ("DTE", "DEUTSCHE TELEKOM N AG", "Communication", "Germany", 45),
    ("BP", "BP PLC", "Energy", "United Kingdom", 40),
    ("RIO", "RIO TINTO PLC", "Materials", "United Kingdom", 40),
    ("REL", "RELX PLC", "Industrials", "United Kingdom", 40),
    ("9984", "SOFTBANK GROUP CORP", "Communication", "Japan", 45),
    ("7974", "NINTENDO LTD", "Communication", "Japan", 45),
    ("8316", "SUMITOMO MITSUI FINANCIAL GROUP INC", "Financials", "Japan", 45),
    ("ABI", "ANHEUSER BUSCH INBEV SA NV", "Consumer Staples", "Belgium", 35),
    ("IBE", "IBERDROLA SA", "Utilities", "Spain", 40),
    ("SAN", "BANCO SANTANDER SA", "Financials", "Spain", 40),
    ("ENEL", "ENEL", "Utilities", "Italy", 30),
    ("ISP", "INTESA SANPAOLO", "Financials", "Italy", 30),
    ("DBS", "DBS GROUP HOLDINGS LTD", "Financials", "Singapore", 35),
    ("1299", "AIA GROUP LTD", "Financials", "Hong Kong", 40),
    ("SHOP", "SHOPIFY SUBORDINATE VOTING INC CLASS A", "Information Technology", "Canada", 0),
    ("CAD", "CAD CASH", "Cash and/or Derivatives", CA, 10),
];

const EMERGING: &[Row] = &[
    ("2330", "TAIWAN SEMICONDUCTOR MANUFACTURING", "Information Technology", "Taiwan", 1050),
    ("700", "TENCENT HOLDINGS LTD", "Communication", "China", 480),
    ("005930", "SAMSUNG ELECTRONICS LTD", "Information Technology", "Korea", 420),
    ("9988", "ALIBABA GROUP HOLDING LTD", "Consumer Discretionary", "China", 340),
    ("000660", "SK HYNIX INC", "Information Technology", "Korea", 200),
    ("HDFCBANK", "HDFC BANK LTD", "Financials", "India", 150),
    ("RELIANCE", "RELIANCE INDUSTRIES LTD", "Energy", "India", 140),
    ("3690", "MEITUAN", "Consumer Discretionary", "China", 110),
    ("1810", "XIAOMI CORP", "Information Technology", "China", 120),
    ("939", "CHINA CONSTRUCTION BANK CORP H", "Financials", "China", 110),
    ("ICICIBANK", "ICICI BANK LTD", "Financials", "India", 100),
    ("INFY", "INFOSYS LTD", "Information Technology", "India", 70),
    ("1211", "BYD LTD H", "Consumer Discretionary", "China", 70),
    ("2317", "HON HAI PRECISION INDUSTRY LTD", "Information Technology", "Taiwan", 90),
    ("2454", "MEDIATEK INC", "Information Technology", "Taiwan", 80),
    ("1398", "INDUSTRIAL AND COMMERCIAL BANK OF CHINA", "Financials", "China", 60),
    ("BHARTIARTL", "BHARTI AIRTEL LTD", "Communication", "India", 60),
    ("PDD", "PDD HOLDINGS INC", "Consumer Discretionary", "China", 60),
    ("VALE3", "VALE SA", "Materials", "Brazil", 45),
    ("PETR4", "PETROBRAS PREF", "Energy", "Brazil", 40),
    ("ITUB4", "ITAU UNIBANCO HOLDING PREF SA", "Financials", "Brazil", 40),
    ("2222", "SAUDI ARABIAN OIL", "Energy", "Saudi Arabia", 50),
    ("1120", "AL RAJHI BANK", "Financials", "Saudi Arabia", 45),
    ("NPN", "NASPERS LTD N", "Consumer Discretionary", "South Africa", 50),
    ("FEMSAUBD", "FOMENTO ECONOMICO MEXICANO UNITS", "Consumer Staples", "Mexico", 25),
    ("WALMEX", "WAL-MART DE MEXICO SAB DE CV", "Consumer Staples", "Mexico", 25),
    ("BBCA", "BANK CENTRAL ASIA TBK", "Financials", "Indonesia", 35),
    ("PTT", "PTT PCL", "Energy", "Thailand", 20),
    ("CAD", "CAD CASH", "Cash and/or Derivatives", CA, 10),
];

pub fn seed_holdings(as_of: NaiveDate) -> Vec<FundHoldings> {
    let tsx = "Toronto Stock Exchange";
    let xeqt = FundHoldings {
        symbol: "XEQT.TO".into(),
        name: Some("iShares Core Equity ETF Portfolio".into()),
        as_of,
        source: "seed".into(),
        rows: vec![
            fund_row("ITOT", "ISHARES CORE S&P TOTAL US STOCK MARKET ETF", 2501, US, "NYSE Arca", "USD"),
            fund_row("XUS", "ISHARES CORE S&P 500 INDEX ETF", 2261, CA, tsx, "CAD"),
            fund_row("XIC", "ISHARES CORE S&P/TSX CAPPED COMPOSITE INDEX ETF", 2464, CA, tsx, "CAD"),
            fund_row("XEF", "ISHARES CORE MSCI EAFE IMI INDEX ETF", 2013, CA, tsx, "CAD"),
            fund_row("XEC", "ISHARES CORE MSCI EMERGING MARKETS IMI INDEX ETF", 737, CA, tsx, "CAD"),
            Constituent {
                ticker: Some("CAD".into()),
                name: "CAD CASH".into(),
                weight: w(24),
                sector: Some("Cash and/or Derivatives".into()),
                country: Some(CA.into()),
                asset_class: AssetClass::Cash,
                currency: Some("CAD".into()),
                exchange: None,
            },
        ],
    };
    let developed: Vec<Row> = DEVELOPED.iter().copied().filter(|r| r.4 > 0).collect();
    vec![
        xeqt,
        fund("XUS.TO", "iShares Core S&P 500 Index ETF", as_of, SP500, "", "USD"),
        fund("VFV.TO", "Vanguard S&P 500 Index ETF", as_of, SP500, "", "USD"),
        fund("ITOT", "iShares Core S&P Total U.S. Stock Market ETF", as_of, US_TOTAL, "", "USD"),
        fund("VTI", "Vanguard Total Stock Market ETF", as_of, US_TOTAL, "", "USD"),
        fund("XIC.TO", "iShares Core S&P/TSX Capped Composite Index ETF", as_of, CANADA, tsx, "CAD"),
        fund("XEF.TO", "iShares Core MSCI EAFE IMI Index ETF", as_of, &developed, "", "EUR"),
        fund("XEC.TO", "iShares Core MSCI Emerging Markets IMI Index ETF", as_of, EMERGING, "", "USD"),
    ]
}

/// The same funds a month earlier, with a few sleeves at different weights and one
/// tail name missing, so the Changes feed has something to report on seed data.
pub fn seed_holdings_earlier(as_of: NaiveDate) -> Vec<FundHoldings> {
    let tweaks: &[(&str, &[(&str, i64)])] = &[
        ("XEQT.TO", &[("XEC", -60), ("XEF", 60)]),
        ("XUS.TO", &[("NVDA", -70), ("AAPL", 45), ("MSFT", 25)]),
        ("VFV.TO", &[("NVDA", -70), ("AAPL", 45), ("MSFT", 25)]),
        ("XIC.TO", &[("SHOP", -80), ("RY", 80)]),
    ];
    let mut out = seed_holdings(as_of);
    for f in &mut out {
        if let Some((_, deltas)) = tweaks.iter().find(|(s, _)| *s == f.symbol) {
            for (t, bp) in *deltas {
                if let Some(r) = f.rows.iter_mut().find(|r| r.ticker.as_deref() == Some(t)) {
                    r.weight += Decimal::new(*bp, 2);
                }
            }
        }
        // The last (smallest) name was not in the fund yet; its weight sat in the first.
        if matches!(f.symbol.as_str(), "XIC.TO" | "ITOT" | "VTI") && f.rows.len() > 2 {
            let gone = f.rows.remove(f.rows.len() - 2);
            f.rows[0].weight += gone.weight;
        }
    }
    out
}

fn fund_row(ticker: &str, name: &str, bp: u32, country: &str, exchange: &str, currency: &str) -> Constituent {
    Constituent {
        ticker: Some(ticker.into()),
        name: name.into(),
        weight: w(bp),
        sector: None,
        country: Some(country.into()),
        asset_class: AssetClass::Equity,
        currency: Some(currency.into()),
        exchange: Some(exchange.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn every_seed_fund_sums_to_100() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        for f in seed_holdings(d).into_iter().chain(seed_holdings_earlier(d)) {
            let c = f.coverage();
            assert!((c - dec!(100)).abs() <= dec!(0.5), "{} sums to {c}", f.symbol);
            assert!(f.rows.iter().all(|r| r.weight > Decimal::ZERO), "{}", f.symbol);
        }
    }
}
