# Access Worthwatch remotely

This page shows you how to open Worthwatch from your phone or another computer without exposing your finances to the internet.

> [!WARNING]
> Worthwatch has no login yet. Anyone who can reach its port sees all your data. Never forward its port from your router. For the full setup and its limits, see [Keep Worthwatch secure](security.md).

Worthwatch also refuses requests addressed to any name it doesn't know, and requests sent by other websites, so a page you visit can't read it through your browser.

## Choose an option

| Option | Who can reach it | Use it when |
|---|---|---|
| Localhost (default) | This computer only | You use Worthwatch on the machine it runs on. |
| [Tailscale](#use-tailscale) (recommended) | The Apple Pay feed from any device on your tailnet; the dashboard from your Tailscale login only | You want it on your phone at home and away. |
| [Your local network](#open-it-on-your-local-network) | Every device on your Wi-Fi, over plain HTTP | You trust everyone on the network, and only need it at home. |

```text
           ┌──────────────── your computer ────────────────┐
 phone ──▶ │ Tailscale ──▶ 127.0.0.1:7777 ──▶ Worthwatch    │
 (tailnet) │  (encrypted, your devices only)               │
           └───────────────────────────────────────────────┘
 router port forward ──✕  never
```

## Use Tailscale

[Tailscale](https://tailscale.com) connects your devices over an encrypted private network. Worthwatch keeps listening on localhost, and Tailscale forwards requests from your other devices to it.

1. [Install Tailscale](https://tailscale.com/download) on the computer that runs Worthwatch, and on your phone. Sign in to the same account on both.

2. Tell Worthwatch your Tailscale login, so it shows you the dashboard. Without it, Worthwatch answers only the Apple Pay feed over Tailscale.

   - **Native on a Mac:**

     ```sh
     WORTHWATCH_TAILSCALE_OWNER=you@example.com ./deploy/macos/install.sh --no-build
     ```

   - **Container:**

     ```sh
     WORTHWATCH_TAILSCALE_OWNER=you@example.com ./deploy/container/run.sh
     ```

   Find your login with `tailscale status --json` (look for `LoginName`).

3. On the computer, share Worthwatch with your tailnet:

   ```sh
   tailscale serve --bg 7777
   ```

   The command prints an address such as `https://my-mac.tailnet-name.ts.net`.

4. On your phone, open that address.

To stop sharing, run:

```sh
tailscale serve reset
```

This works the same for the binary and for a container, because both listen on `127.0.0.1:7777` by default. `tailscale serve` passes your login to Worthwatch; other logins on your tailnet, and anyone reaching it through Tailscale Funnel, still get only the Apple Pay feed.

## Open it on your local network

Use this only on a network you trust. Every device on it can then read and change your data, and the traffic isn't encrypted, so anyone on the Wi-Fi can also watch it. Prefer Tailscale.

- **Binary:** listen on all interfaces.

  ```sh
  WORTHWATCH_BIND=0.0.0.0:7777 ./worthwatch
  ```

- **Docker:** publish the port on all interfaces.

  ```sh
  WORTHWATCH_HOST=0.0.0.0 docker compose -f deploy/docker-compose.yml up -d
  ```

  To use another host port, also set `WORTHWATCH_PORT`, for example `WORTHWATCH_PORT=8080`.

Then open `http://<computer-ip>:7777` from another device. Worthwatch logs a warning at startup while it listens beyond `127.0.0.1`.

To reach it by a name such as `mini.local` instead of an IP address, add the name to `WORTHWATCH_ALLOWED_HOSTS`, for example `WORTHWATCH_ALLOWED_HOSTS=mini.local`. Worthwatch refuses names it doesn't know with `421`.

## Keep your data safe

- Your data lives in one folder: `WORTHWATCH_DATA`, or `/data` in Docker. It's plain YAML.
- Worthwatch only fetches public prices and rates. Nothing about your accounts leaves the machine unless you choose the cloud chat model.
- Copy backups off the machine. See [Back up and restore](backup-restore.md).
