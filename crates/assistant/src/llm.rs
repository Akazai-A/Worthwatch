//! The model boundary. A model gets a [`Prompt`] (schema, today's date, the account
//! ids/names/kinds/currencies and held symbols; never a balance or a value) and
//! answers with text that [`parse_reply`] must read as a [`Proposal`], or the reply
//! is rejected. [`Manual`] is the fixed-answer impl for tests; [`Off`] never answers.

use thiserror::Error;

use crate::{Context, Proposal};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Prompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("{0} is not reachable: {1}")]
    Unreachable(&'static str, String),
    #[error("{0}")]
    Http(String),
    #[error("{provider}: could not read the reply: {reason}")]
    Parse { provider: &'static str, reason: String },
    #[error("no API key: set ANTHROPIC_API_KEY in the server's environment")]
    NoKey,
    #[error("no model is configured")]
    Off,
}

pub trait Llm {
    fn name(&self) -> &'static str;
    /// The raw text of the model's reply.
    fn complete(&self, prompt: &Prompt) -> Result<String, LlmError>;
}

/// Fixed reply; the impl used in tests.
#[derive(Debug, Clone, Default)]
pub struct Manual {
    pub reply: String,
}

impl Llm for Manual {
    fn name(&self) -> &'static str {
        "manual"
    }
    fn complete(&self, _prompt: &Prompt) -> Result<String, LlmError> {
        Ok(self.reply.clone())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Off;

impl Llm for Off {
    fn name(&self) -> &'static str {
        "off"
    }
    fn complete(&self, _prompt: &Prompt) -> Result<String, LlmError> {
        Err(LlmError::Off)
    }
}

const SCHEMA: &str = r#"You turn one message about personal finances into JSON for an app that will check it and ask the user to approve. You never write anything yourself. Reply with one JSON object and nothing else:

{"items":[...], "date": "YYYY-MM-DD" or null, "query": string or null, "answer": string or null, "unparsed": [strings]}

Each item is one of:
{"kind":"balance","account":ID,"value":NUMBER,"source":PHRASE}        the account's new balance in its own currency; for a liability (credit card, mortgage, loan) the amount owed
{"kind":"adjust","account":ID,"delta":NUMBER,"source":PHRASE}         a relative change ("up 500" = 500, "down 200" = -200; a payment that reduces what is owed on a liability is negative)
{"kind":"trade","account":ID or null,"symbol":TICKER,"units":NUMBER,"price":NUMBER or null,"source":PHRASE}   units signed: bought +, sold -; account null when the message does not say
{"kind":"price","symbol":TICKER,"price":NUMBER,"source":PHRASE}       a quoted price for a held symbol
{"kind":"new_account","name":STRING,"account_kind":KIND,"currency":CODE or null,"value":NUMBER or null,"source":PHRASE}
{"kind":"note","note":STRING,"source":PHRASE}                          something to remember on this date
{"kind":"spend","what":STRING,"amount":NUMBER,"paid_with":ID or "cash" or null,"category":STRING or null,"source":PHRASE}   money spent, for the budget ("coffee 6 cash", "lunch 18 on visa"); not a balance

ID is an account id from the list below. KIND is one of chequing savings tfsa rrsp fhsa resp non_registered crypto property vehicle other_asset credit_card line_of_credit mortgage loan other_liability.
"source" is the exact words of the message the item came from.
"18.5k" means 18500. "down to 640" is a balance of 640; "down 640" is an adjust of -640.
A purchase ("spent", "paid", or a thing, an amount and how it was paid) is a "spend", never a balance or an adjust. paid_with null means cash.
A question about the user's numbers is a "query": "net worth", "cash", "investments", "property", "debt", or an account id; you do not know any balances, the app answers from its data. Anything else to say goes in "answer". Put any part of the message you cannot turn into an item into "unparsed" rather than guessing. Omit "date" unless the message names one."#;

/// What a model sees. Identifiers only: ids, names, kinds, currencies, symbols, today.
pub fn build_prompt(ctx: &Context, text: &str) -> Prompt {
    let mut system = String::from(SCHEMA);
    system
        .push_str(&format!("\n\nToday is {}. Base currency {}.\nAccounts:\n", ctx.today, ctx.base_currency));
    for a in &ctx.accounts {
        let kind = format!("{:?}", a.kind).to_ascii_lowercase();
        let kind = kind.replace("nonregistered", "non_registered").replace("creditcard", "credit_card");
        let kind = kind.replace("lineofcredit", "line_of_credit").replace("otherasset", "other_asset");
        let kind = kind.replace("otherliability", "other_liability");
        system.push_str(&format!("- {} \"{}\" {kind} {}", a.id, a.name, a.currency));
        if let Some(i) = &a.institution {
            system.push_str(&format!(" at {i}"));
        }
        if let Some(ps) = &a.positions {
            let syms: Vec<&str> = ps.iter().map(|p| p.symbol.as_str()).collect();
            system.push_str(&format!(" holds [{}]", syms.join(", ")));
        }
        system.push('\n');
    }
    Prompt { system, user: text.trim().to_string() }
}

/// The reply as a proposal, tolerating code fences and chatter around the object.
pub fn parse_reply(text: &str) -> Result<Proposal, String> {
    let t = text.trim();
    let start = t.find('{').ok_or("no JSON object in the reply")?;
    let end = t.rfind('}').ok_or("no JSON object in the reply")?;
    if end < start {
        return Err("no JSON object in the reply".into());
    }
    serde_json::from_str::<Proposal>(&t[start..=end])
        .map_err(|e| format!("reply is not a valid proposal: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::ctx;

    #[test]
    fn the_prompt_carries_identifiers_and_never_a_balance() {
        let p = build_prompt(&ctx(), "td chq 5102");
        assert!(p.system.contains("- td-chq \"TD Chequing\" chequing CAD at TD"));
        assert!(p
            .system
            .contains("- ws-tfsa \"Wealthsimple TFSA\" tfsa CAD at Wealthsimple holds [XEQT.TO, VFV.TO]"));
        assert!(p.system.contains("Today is 2026-11-02"));
        assert!(!p.system.contains("4273"), "balances never leave the machine");
        assert!(!p.system.contains("412000"));
        assert_eq!(p.user, "td chq 5102");
    }

    #[test]
    fn replies_are_read_strictly_but_fences_are_tolerated() {
        let ok = "```json\n{\"items\":[{\"kind\":\"balance\",\"account\":\"td-chq\",\"value\":5102}]}\n```";
        assert_eq!(parse_reply(ok).unwrap().items.len(), 1);
        let chatty = "Sure! Here you go: {\"items\":[],\"answer\":\"hi\"} Hope that helps.";
        assert_eq!(parse_reply(chatty).unwrap().answer.as_deref(), Some("hi"));
        assert!(parse_reply("I cannot do that").unwrap_err().contains("no JSON"));
        assert!(parse_reply("{\"items\":[{\"kind\":\"balance\",\"account\":\"x\"}]}")
            .unwrap_err()
            .contains("not a valid proposal"));
        assert!(parse_reply("{\"items\":[{\"kind\":\"delete_everything\"}]}").is_err());
    }
}
