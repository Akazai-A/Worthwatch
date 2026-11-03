# Configure Worthwatch

You configure Worthwatch in two places:

- **`ledger.yaml`**, under `settings`, for anything about your money: base currency, data sources, the chat model, and plan assumptions.
- **Environment variables** for anything about the process: where data lives and which address it listens on.

Edit `ledger.yaml` in any text editor. The app reloads when you save.

## Environment variables

| Variable | Default | What it does |
|---|---|---|
| `WORTHWATCH_DATA` | `data` (`/data` in Docker) | Folder for `ledger.yaml`, `quotes.json`, `holdings/` and `backups/`. |
| `WORTHWATCH_BIND` | `127.0.0.1:7777` | Address and port the server listens on. See [Access Worthwatch remotely](remote-access.md). |
| `WORTHWATCH_ALLOWED_HOSTS` | unset | Extra host names to accept, comma separated. `localhost`, IP addresses and `*.ts.net` are always accepted; any other name gets `421`. |
| `WORTHWATCH_TAILSCALE_OWNER` | unset | Tailscale logins that may open the whole app through `tailscale serve`, comma separated. Unset: only the Apple Pay feed answers over Tailscale. See [Keep Worthwatch secure](security.md). |
| `WORTHWATCH_TAPS_DIR` | iCloud Drive's `Shortcuts/Worthwatch` | Folder the iCloud Apple Pay feed reads. In a container, mount only that folder, read-only. |
| `WORTHWATCH_OFFLINE` | unset | Set to `1` to never fetch prices, FX or holdings, and never ask a model. |
| `WORTHWATCH_URL` | from `WORTHWATCH_BIND` | Where `worthwatch mcp` finds the running app. |
| `WORTHWATCH_OLLAMA_URL` | `http://127.0.0.1:11434` | Ollama address. `settings.assistant.ollama_url` takes priority. |
| `ANTHROPIC_API_KEY` | unset | Turns on the cloud chat model when `settings.assistant.provider` is `cloud`. |

Docker Compose adds two variables for the host side of the port mapping:

| Variable | Default | What it does |
|---|---|---|
| `WORTHWATCH_HOST` | `127.0.0.1` | Host address the container's port is published on. Any other value lets every device on your network read and change your data. |
| `WORTHWATCH_PORT` | `7777` | Host port. |

## Set your base currency

Worthwatch values everything in one currency. Set it once:

```yaml
settings:
  base_currency: CAD
```

The Bank of Canada FX source only covers rates against CAD. If you choose another base currency, set `fx: off` and type rates yourself. See [Prices, FX and fund holdings](data-sources.md#type-a-rate-yourself).

## Choose price and FX sources

```yaml
settings:
  providers:
    prices: yahoo          # yahoo | stooq | off
    fx: bank_of_canada     # bank_of_canada | off
```

For details on each source, see [Prices, FX and fund holdings](data-sources.md).

## Choose the chat model

```yaml
settings:
  assistant:
    provider: ollama       # ollama | cloud | shorthand
    # ollama_url: http://127.0.0.1:11434
    # model: llama3.1
```

| Provider | What reads your message | What leaves your machine |
|---|---|---|
| `ollama` (default) | A local model in [Ollama](https://ollama.com). If Ollama isn't running, the built-in parser answers and the card says so. | Nothing. |
| `cloud` | Claude, through the Anthropic API. Needs `ANTHROPIC_API_KEY`. | Account IDs, names, kinds, currencies and symbols. Never a balance. You see the exact text and select **Send** first. |
| `shorthand` | The built-in parser only. | Nothing. |

### Run Ollama with Docker

The Compose file has an optional Ollama service that the app already points at.

1. Start it with the `ai` profile:

   ```sh
   docker compose -f deploy/docker-compose.yml --profile ai up -d
   ```

2. Pull a model once:

   ```sh
   docker compose -f deploy/docker-compose.yml exec ollama ollama pull llama3.1
   ```

## Set up plan inputs

The Plan screen reads recurring rules, assumptions and contribution room from the ledger. You can edit all of them on the Plan screen too.

```yaml
recurring:
  - { account: ws-tfsa, amount: 500, every: month, note: auto-deposit }   # week | two_weeks | month | year
  - { account: td-mortgage, amount: 1350, every: two_weeks }              # a payment lowers the debt

settings:
  projection:
    return_pct: 6          # assumed yearly return on investments
    cash_pct: 0
    property_pct: 2
    target: 1500000
    rates: { td-mortgage: 4.9 }   # interest rate per debt account

room:                      # the figure CRA My Account or your notice of assessment shows
  tfsa: { as_of: 2026-01-01, amount: 13500 }
  rrsp: { as_of: 2026-05-01, amount: 18400 }

contributions:             # money you put in; negative is a withdrawal
  - { date: 2026-02-15, account: ws-tfsa, amount: 500 }
```

- **Recurring rules are plans.** They drive the projection and the "planned" part of a room bar.
- **Contributions are facts.** Only logged contributions count as used room.
- **Room starts from CRA's figure.** TFSA adds each new January's limit plus last year's withdrawals. FHSA adds $8,000 a year. RRSP adds nothing, so update it from each notice of assessment.

## Account kinds

Each account has a `kind`. The kind decides its group on charts and which room it counts against.

| Group | Kinds |
|---|---|
| Cash | `chequing`, `savings` |
| Investments | `tfsa`, `rrsp`, `fhsa`, `resp`, `non_registered`, `crypto` |
| Property | `property`, `vehicle`, `other_asset` |
| Debt | `credit_card`, `line_of_credit`, `mortgage`, `loan`, `other_liability` |

Enter debts as the positive amount you owe.
