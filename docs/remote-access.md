# Access Worthwatch remotely

This page shows you how to open Worthwatch from your phone or another computer without exposing your finances to the internet.

> [!WARNING]
> Worthwatch has no login yet. Anyone who can reach its port sees all your data. Never forward its port from your router, and never use Tailscale Funnel with it.

## Choose an option

| Option | Who can reach it | Use it when |
|---|---|---|
| Localhost (default) | This computer only | You use Worthwatch on the machine it runs on. |
| [Tailscale](#use-tailscale) (recommended) | Your own devices, from anywhere | You want it on your phone at home and away. |
| [Your local network](#open-it-on-your-local-network) | Every device on your Wi-Fi | You trust everyone on the network, and only need it at home. |

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

2. On the computer, share Worthwatch with your tailnet:

   ```sh
   tailscale serve --bg 7777
   ```

   The command prints an address such as `https://my-mac.tailnet-name.ts.net`.

3. On your phone, open that address.

To stop sharing, run:

```sh
tailscale serve reset
```

This works the same for the binary and for Docker, because both listen on `127.0.0.1:7777` by default.

## Open it on your local network

Use this only on a network you trust. Every device on it can then open your dashboard.

- **Binary:** listen on all interfaces.

  ```sh
  WORTHWATCH_BIND=0.0.0.0:7777 ./worthwatch
  ```

- **Docker:** publish the port on all interfaces.

  ```sh
  WORTHWATCH_HOST=0.0.0.0 docker compose -f deploy/docker-compose.yml up -d
  ```

  To use another host port, also set `WORTHWATCH_PORT`, for example `WORTHWATCH_PORT=8080`.

Then open `http://<computer-ip>:7777` from another device.

## Keep your data safe

- Your data lives in one folder: `WORTHWATCH_DATA`, or `/data` in Docker. It's plain YAML.
- Worthwatch only fetches public prices and rates. Nothing about your accounts leaves the machine unless you choose the cloud chat model.
- Copy backups off the machine. See [Back up and restore](backup-restore.md).
