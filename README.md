## ODD-BOX

A simple, cross-platform reverse proxy server tailored for local development. Think of it as a lightweight alternative to something like IIS or Caddy, with both TUI & GUI modes, backed by declarative TOML configuration.

Under the hood, odd-box is a thin wrapper around the [cruma](https://cruma.io) agent library, which handles all proxy, tunnel, TLS termination, ACME, and process-hosting logic. odd-box adds its own branding, CLI, and opinionated defaults on top.

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases).

**NOTE**: If you do not have a specific reason for using odd-box, it is recommended you migrate to [cruma](https://cruma.io/#downloads) as it is more actively developed and maintained. 
