[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml)

## ODD-BOX

A simple, cross-platform reverse proxy server tailored for local development and tinkering. Think of it as a lightweight alternative to something like IIS or Caddy, with declarative YAML configuration and built-in process management.

odd-box manages your backend services, routes traffic by hostname, terminates TLS with automatic certificate generation, and keeps your processes running — all from a single config file that's easy to share and reproduce.

Under the hood, odd-box is a thin wrapper around the [cruma](https://cruma.io) agent library, which handles all proxy, tunnel, TLS termination, ACME, and process-hosting logic. odd-box adds its own branding, CLI, and opinionated defaults on top.

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases).

You can also build it yourself, or install it using brew, cargo, nix, or devbox — see the [installation](#installation) section.

## Features

> See the [docs](https://odd-box.cruma.io) for details and more features.

- **Cross-platform** — Windows, Linux, macOS
- **Three UI modes** — graphical (GUI), terminal (TUI), or headless
- **YAML configuration** — simple, declarative, easy to share
- **Process management** — keep specified binaries running automatically
- **Reverse proxy** — route traffic to local or remote backends by hostname
- **Static file hosting** — serve local directories with optional markdown rendering
- **Automatic TLS** — self-signed certificates generated on first access
- **Let's Encrypt** — ACME TLS-ALPN-01 and DNS-01 for production certificates
- **HTTP/1.1 & HTTP/2** — terminating proxy (layer 7)
- **Start on request** — lazy process startup on first incoming request
- **Idle timeouts** — automatically stop idle processes
- **Middlewares** — rate limiting, CORS, header injection, redirects, URL rewriting, basic auth, API key auth, form auth, and more
- **Config hot-reload** — changes are picked up without restarting
- **Self-update** — built-in `--update` command for manual installs
- **Optional cruma tunnel** — expose local services externally via [cruma.io](https://cruma.io) (or run fully local with `local_only: true`)

## Screenshot

**odd-box GUI:**

![Screenshot of odd-box](/ob3.png)

## Getting Started

Generate a starter configuration file:

```
odd-box --init
```

This creates an `odd-box.yaml` that looks like this:

```yaml
tunnel_id: ANON
tunnel_secret: ANON
local_only: true

listeners:
  - port: 8080
    addr: localhost
    tls: false
  - port: 4343
    addr: localhost
    tls: true

backends: []
processes: []
frontends: []
```

Then run odd-box:

```
odd-box
```

By default odd-box launches the GUI. You can also choose a UI mode explicitly:

```
odd-box --gui        # graphical interface (default when a display is available)
odd-box --tui        # terminal interface
odd-box --headless   # no UI, just the proxy
```

Open your browser at `https://localhost:4343` to access the web interface and configure services.

## Configuration

odd-box uses YAML configuration. Here's a more complete example:

```yaml
# odd-box configuration
tunnel_id: ANON
tunnel_secret: ANON
local_only: true

listeners:
  - port: 8080
    addr: localhost
    tls: false
  - port: 4343
    addr: localhost
    tls: true

# Backends define upstream targets
backends:
  # Reverse proxy to a running service
  - id: my-api
    kind: http
    destination: "localhost:3000"

  # Serve a local directory
  - id: docs
    kind: local-directory
    destination: /home/user/docs
    allow_directory_indexing: true
    render_markdown: true

# Processes that odd-box manages (start/stop/restart)
processes:
  - id: my-app
    command: node
    args: [server.js]
    working_directory: /home/user/my-app
    auto_start: true
    start_on_request: true
    idle_timeout_seconds: 300
    env:
      NODE_ENV: development

  - id: python-server
    command: python
    args: ["-m", "http.server", "9000"]
    auto_start: false

# Frontends map hostnames to backends or processes
frontends:
  - hostname: my-api.localhost
    process_id: my-app

  - hostname: docs.localhost
    backend_id: docs

  - hostname: py.localhost
    backend_id: python-server

# Global environment variables for all hosted processes
global_env:
  RUST_LOG: info
```

### Configuration options

Run `odd-box --config-schema` to print the full JSON schema for all available configuration options, including middlewares, authentication, restart policies, and certificate modes.

### Specifying a config file

```
odd-box --config path/to/my-config.yaml
```

If no `--config` is given, odd-box looks for `odd-box.yaml`, `oddbox.yaml`, `odd-box.yml`, `oddbox.yml`, or `config.yaml` in the current directory.

## Installation

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases).

### Homebrew (macOS / Linux)

```sh
brew tap OlofBlomqvist/repo
brew install odd-box
```

### Cargo

```sh
cargo install odd-box
```

### Nix

```sh
nix build github:OlofBlomqvist/odd-box
```

### Nix Flake

```nix
{
  description = "example flake with odd-box";
  inputs = {
    oddbox.url = "github:OlofBlomqvist/odd-box";
  };
  # ...
}
```

### Devbox

```json
{
  "packages": [
    "github:OlofBlomqvist/odd-box"
  ]
}
```

## Migrating from odd-box v1/v2 (TOML config)

If you have an existing odd-box configuration in the old TOML or V4 YAML format, use the built-in migration tool:

```sh
odd-box --migrate old-config.toml > odd-box.yaml
```

Review the generated `odd-box.yaml`, then start odd-box normally. The migration tool handles conversion of hosted processes, remote targets, directory servers, listeners, and tunnel credentials.

## Self-Update

For manual installs (not managed by a package manager):

```
odd-box --update
```

For package-managed installs, odd-box will tell you to use your package manager instead (e.g. `brew upgrade odd-box`).

## CLI Reference

```
odd-box [OPTIONS]

Options:
  -c, --config <FILE>       Path to configuration file (YAML)
      --gui                 Run the graphical user interface
      --tui                 Run the terminal user interface
      --headless            Run in headless mode (no UI)
      --update              Run self-update
      --init                Initialize a new config file
      --migrate <OLD_CONFIG> Migrate an old config to the new format
      --config-schema       Print JSON schema for the config format
      --theme <MODE>        Theme: light, dark, system
      --tower-server <ADDR> Tower server address [default: tower.cruma.io:443]
      --protocol <PROTO>    Transport protocol: auto, quic, h2 [default: auto]
  -h, --help                Print help
  -V, --version             Print version
```

## Documentation

For more in-depth guidance, see the [documentation](https://odd-box.cruma.io).

## License

See [LICENSE](LICENSE) for details.