## Configuration (V3) 
 
 
### Quick start

Create `odd-box.toml` (or `Config.toml`) and run:

```bash
./odd-box              # loads odd-box.toml or Config.toml in the cwd
./odd-box "/path/to/config.toml"   # explicit path
```

Most settings can be reloaded at runtime through the admin API.

---

### Placeholder variables

| Variable | Resolves to | Default when unset | What it’s for |
|----------|-------------|--------------------|---------------|
| `$root_dir` | Value of `root_dir` | current working directory | Use in any path or argument to stay portable. |
| `$cfg_dir`  | Directory of the loaded config file | — | Handy for relative paths that follow the config. |
| `$port`     | The port chosen for the current **hosted_process** | next free ≥ `port_range_start` | Lets you inject the runtime port into args/env without hard‑coding. |

> **Tip (VS Code):** Install *Even Better TOML* and keep the `#:schema …` tag for instant validation & IntelliSense.

---

## 1 — Global settings

### What each field does

| Field | Meaning | Default |
|-------|---------|---------|
| `version` | Must be `"V3"` so odd‑box knows which schema to parse. | — |
| `root_dir` | Base for `$root_dir`; relative paths are resolved from here. | current working dir |
| `ip` | Address the HTTP/HTTPS listeners bind to. | `127.0.0.1` |
| `http_port` / `tls_port` | Public ports for HTTP and HTTPS listeners. | `8080` / `4343` |
| `alpn` | Offer HTTP/2 over TLS via ALPN; disable only for exotic setups. | `true` |
| `port_range_start` | First port odd‑box tries for **hosted_process** auto‑ports. | `4200` |
| `default_log_format` | Line layout for process logs (`standard` or `dotnet`). | `standard` |
| `log_level` | Odd‑box’s own log verbosity. | `Info` |
| `auto_start` | Whether to start every hosted process at launch unless they override. | `true` |
| `use_loopback_ip_for_procs` | Always proxy to `127.0.0.1` instead of the incoming host name (avoids IPv6/SNI weirdness). | `true` |
| `env_vars` | Key–value pairs injected into **every** hosted process. | `[]` |
| `lets_encrypt_account_email` | Enables Let’s Encrypt support; this email is sent to ACME. | unset |
| `odd_box_url` / `odd_box_password` | Custom hostname + password for the admin UI/API; if unset, UI binds to *localhost* and is unsecured. | unset |

### Example block

```toml
#:schema https://raw.githubusercontent.com/OlofBlomqvist/odd-box/main/odd-box-schema-v3.1.json
version                   = "V3"
root_dir                  = "/srv/odd-box"
ip                        = "0.0.0.0"
http_port                 = 8080
tls_port                  = 4343
alpn                      = true
port_range_start          = 4200
default_log_format        = "standard"
log_level                 = "Info"
auto_start                = true
use_loopback_ip_for_procs = true
env_vars = [
  { key = "GLOBAL_KEY", value = "value" }
]
lets_encrypt_account_email = "admin@example.com"
odd_box_url                = "admin.example.com"
odd_box_password           = "s3cr3t"
```

---

## 2 — Reverse‑proxy back‑ends (`[[remote_target]]`)

### Field meanings

| Field | What it controls | Default |
|-------|------------------|---------|
| `host_name` | Incoming host (SNI / Host header) odd‑box listens for. **Required**. | — |
| `capture_subdomains` | If `true`, `*.host_name` is also routed here. | `false` |
| `forward_subdomains` | If `true`, the matching subdomain is kept when rewriting Host header (`foo.api.example.com` ➜ `foo.backend`). | `false` |
| `terminate_tls` | Terminate HTTPS at odd‑box, forward HTTP to the back‑end. | `false` |
| `terminate_http` | Force layer‑7 proxy even for plain HTTP (can enable URL rewriting). | `false` |
| `redirect_to_https` | Respond with 308 to HTTPS listener. | `false` |
| `enable_lets_encrypt` | Issue real certs for `host_name` via Let’s Encrypt. Requires `lets_encrypt_account_email`. | `false` |
| `keep_original_host_header` | Forwards inbound `Host` header unchanged instead of using the back‑end’s address. | `false` |
| `backends` | Array of one or more servers. **Each needs `address` + `port`.** | — |

Back‑end object keys:

| Key | Meaning | Default |
|-----|---------|---------|
| `address` | IP / FQDN of the target server. | — |
| `port` | TCP port on the target. | — |
| `https` | If `true`, use TLS to the back‑end. | `false` |
| `hints` | List of `H1`, `H2`, `H2C`, `H2CPK`, `H3` to steer protocol negotiation. | `[]` |

### Example

```toml
[[remote_target]]
host_name = "api.example.com"
redirect_to_https         = true
capture_subdomains        = false
forward_subdomains        = true
terminate_tls             = true
enable_lets_encrypt       = true
keep_original_host_header = true

backends = [
  { address = "10.0.0.5", port = 443, https = true, hints = ["H2","H1"] },
  { address = "10.0.0.6", port = 8443, https = true }
]
```

---

## 3 — Hosted processes (`[[hosted_process]]`)

### Field meanings

| Field | What it controls | Default |
|-------|------------------|---------|
| `host_name` | Incoming host odd‑box proxies to this process. **Required**. | — |
| `dir` | Working directory for the process. | current working dir |
| `bin` | Executable or script to run. **Required**. | — |
| `args` | Command‑line arguments array. | `[]` |
| `env_vars` | Extra env vars (merged with global `env_vars`). | `[]` |
| `log_format` | Overrides `default_log_format`. | inherited |
| `log_level` | Overrides global `log_level`. | inherited |
| `auto_start` | Start with odd‑box? Overrides global `auto_start`. | inherited (`true`) |
| `exclude_from_start_all` | If `true`, `start_all` CLI/API skips this process. | `false` |
| `redirect_to_https` | Respond with 308 to HTTPS listener. | `false` |
| `port` | Fixed port; if unset odd‑box auto‑assigns. | auto‑assign |
| `https` | If `true`, the process itself speaks HTTPS. | `false` |
| `capture_subdomains` | Handle `*.host_name`. | `false` |
| `terminate_http` | Terminates HTTP connections rather than directly forwarding the data stream.  | `false` |
| `terminate_tls` | Terminates TLS connections rather than directly forwarding the data stream.  | `false` |
| `forward_subdomains` | Preserve subdomain when rewriting Host. | `false` |
| `enable_lets_encrypt` | Issue certs for this process’s host. | `false` |
| `hints` | Protocol hints exactly like in `backends`. | `[]` |

### Example

```toml
[[hosted_process]]
host_name              = "app.local"
redirect_to_https         = true
dir                    = "$root_dir/apps/myapp"
bin                    = "./start-server"
args                   = ["--config", "$cfg_dir/app.toml"]
env_vars               = [
  { key = "APP_ENV", value = "production" }
]
log_format             = "dotnet"
log_level              = "Debug"
port                   = 3000
https                  = true
auto_start             = true
exclude_from_start_all = false
capture_subdomains     = false
forward_subdomains     = false
terminate_tls          = false
terminate_http         = false
enable_lets_encrypt    = false
hints                  = ["H1","H2"]
```

---

## 4 — Static file servers (`[[dir_server]]`)

### Field meanings

| Field | What it controls | Default |
|-------|------------------|---------|
| `host_name` | Hostname odd‑box listens for. **Required**. | — |
| `dir` | Directory on disk to serve. **Required**. | — |
| `capture_subdomains` | Serve `*.host_name`. | `false` |
| `enable_directory_browsing` | List files if no `index.html`/`index.md`. | `false` |
| `render_markdown` | Convert `.md` ➜ HTML automatically. | `false` |
| `redirect_to_https` | Respond with 308 to HTTPS listener. | `false` |
| `enable_lets_encrypt` | Issue certs for this site. | `false` |
| `cache_control_max_age_in_seconds` | Sets the cache-control header max-age (public, max-age=<n>, immutable) | `no cache-control header` |`

### Example

```toml
[[dir_server]]
host_name                 = "static.example.com"
dir                       = "/var/www/public"
capture_subdomains        = true
enable_directory_browsing = true
render_markdown           = true
redirect_to_https         = true
enable_lets_encrypt       = true
cache_control_max_age_in_seconds = 60
```
 