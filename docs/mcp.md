# Connect an AI assistant

Worthwatch includes an [MCP](https://modelcontextprotocol.io) server. When you connect it to an AI assistant, you can ask about your net worth and portfolio, and the assistant can propose updates for you to approve.

`worthwatch mcp` is a standard MCP server that runs as a local command and talks over stdio. It works with any MCP client that can start a local command, including Claude Desktop, Claude Code and Cursor. This page uses Claude Desktop as the main example.

The assistant never writes to your ledger. Every proposal waits in the Worthwatch Chat screen until you select **Approve**.

## How it works

```text
MCP client ─────stdio──▶ worthwatch mcp ──HTTP──▶ running Worthwatch app ──▶ ledger.yaml
                                                         │
                                       proposals wait here as cards in Chat
                                       until you approve them in the browser
```

`worthwatch mcp` is a small client of the running app. It reads the same numbers as the dashboard, so the app must be running.

## Before you begin

- Worthwatch is running and you can open it in a browser.
- An MCP client is installed, such as [Claude Desktop](https://claude.ai/download), Claude Code or Cursor.

## Add Worthwatch to Claude Desktop

1. In Claude Desktop, open **Settings** > **Developer** and select **Edit Config**. This opens `claude_desktop_config.json`.

   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Windows: `%APPDATA%\Claude\claude_desktop_config.json`

2. Add a `worthwatch` entry under `mcpServers`. Choose the entry that matches how you run Worthwatch.

   **Binary:** replace the path with where your `worthwatch` binary is.

   ```json
   {
     "mcpServers": {
       "worthwatch": {
         "command": "/path/to/worthwatch",
         "args": ["mcp"],
         "env": { "WORTHWATCH_URL": "http://127.0.0.1:7777" }
       }
     }
   }
   ```

   **Docker:** replace the path with where you cloned the repository.

   ```json
   {
     "mcpServers": {
       "worthwatch": {
         "command": "docker",
         "args": ["compose", "-f", "/path/to/worthwatch/deploy/docker-compose.yml",
                  "exec", "-T", "worthwatch", "/worthwatch", "mcp"]
       }
     }
   }
   ```

3. Save the file and restart Claude Desktop.

4. Start a new chat and ask: "What is my net worth?" Claude asks permission to use the `get_net_worth` tool the first time.

## Add Worthwatch to Claude Code

Run this once, with the path to your `worthwatch` binary:

```sh
claude mcp add worthwatch -e WORTHWATCH_URL=http://127.0.0.1:7777 -- /path/to/worthwatch mcp
```

Run `claude mcp list` to check that `worthwatch` is connected.

## Add Worthwatch to another MCP client

Most clients, including Cursor, take the same `mcpServers` entry as Claude Desktop. Use the binary or Docker entry from [Add Worthwatch to Claude Desktop](#add-worthwatch-to-claude-desktop) in your client's MCP config file.

The client must run on the same computer as Worthwatch, or on one that can reach it, because it starts `worthwatch mcp` as a local process. A chatbot that runs only in the cloud can't start a local command, so it can't use this server.

## Tools

| Tool | What it does |
|---|---|
| `get_net_worth` | Net worth today, totals per group, and the change this year and since the last check-in. |
| `get_accounts` | Every account with its current value, as-of date and positions. |
| `get_portfolio` | Investments by security: units, price, value, weight, gain and day change. |
| `get_exposure` | Look-through exposure by sector, region, country, asset class and currency, plus top companies. |
| `propose_update` | Proposes changes. Writes nothing. The proposal appears as a card in Chat. |
| `check_proposal` | Reports whether you approved or discarded a proposal. |

## Approve a proposal

When Claude calls `propose_update`, the Chat item in the Worthwatch sidebar shows a count.

1. Open **Chat** in Worthwatch.
2. Review the card. It shows each change and its effect on net worth.
3. Select **Approve** to save, or **Discard** to drop it.

Pending proposals live in memory. If you restart Worthwatch, they are dropped.

## Troubleshoot

- **The assistant says it can't reach Worthwatch.** Check that the app is running and that `WORTHWATCH_URL` matches its address. If you changed `WORTHWATCH_BIND`, update the URL too.
- **The tools don't appear.** Check the config file is valid JSON, then restart your client.
