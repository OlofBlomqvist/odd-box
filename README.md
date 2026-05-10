## ODD-BOX

A simple, cross-platform web/proxy server tailored for local development. Think of it as a lightweight alternative to something like IIS or Caddy, with both TUI & GUI modes, backed by declarative TOML configuration.

Under the hood, odd-box is a thin wrapper around the [cruma](https://cruma.io) SDK. Odd-box adds features such as multi-profile management and opts in to the service-map feature by default.

If you do not have a specific reason for using odd-box, you most likely should use its parent project [Cruma](https://cruma.io/#downloads), as it is more performant and actively developed.

### Features

- GUI,TUI & Headless mode
- Process Orchestration
- Kubernetes Integration
- HTTP/1/2/3
- Middeware : ratelimits, auth, rewrites any many more.
- Built in Oauth2 Server
- Request Inspection Interface
- Global Ingress (think ngrok)
- Path based routing
- Automatic ACME (LetsEncrypt/ZeroSSL/etc) certificate issuence
- Self-Signed certs for local development
- Visual Service Map (odd-box specific feature)
- Config-File switch at runtime and general management (odd-box specific feature)

.. And many more! 
