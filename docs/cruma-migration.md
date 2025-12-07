Cruma migration notes
=====================

Goal: replace odd-box networking (listeners, TLS, Hyper dispatch) with cruma_proxy_lib. This doc captures mapping assumptions and gaps while we scaffold the new path.

Current scope (MVP for parity)
- HTTP/TLS listeners on configured http_port/tls_port.
- Host routing for hosted_process, remote_target, dir_server.
- Preserve odd-box admin API hostname handling (odd_box_url plus localhost/odd-box.localhost).
- Use cruma_proxy_lib Terminate-for-HTTP for HTTPS and HTTP listeners; no TCP passthrough yet.
- ACME/self-signed: currently SelfSigned; revisit LetsEncrypt/Acme when wiring cert resolver.

Mapping sketch
- Listener HTTP: cruma types::Listener::Http { port: http_port, routes: all http routes } (optional port offset for side-by-side runs).
- Listener TLS: Listener::Tls { port: tls_port, routes: [ one catch-all TerminateForHTTP with routes driven by host rules ] }.
- Host match: Odd-box host_name => HostPattern::Exact; capture_subdomains => HostPattern::Base.
- Backends:
  - hosted_process: backend id `hosted::<host_name>`, endpoints from active_port/port/https hint, host addr 127.0.0.1 or localhost depending on use_loopback_ip_for_procs.
  - remote_target: backend id `remote::<host_name>`, endpoints from backends entries, https flag => Protocol::Https.
  - dir_server: placeholder 501 response for now; needs real static hosting mapping.
  - admin/web ui: placeholder 501 response for now.
- TLS passthrough: not mapped yet (cruma tcp_backends left empty); needs follow-up if odd-box TCP tunnel mode must remain.

Open questions / follow-ups
- How to surface connection tracking/statistics to odd-box UI from cruma pipeline.
- LetsEncrypt integration vs cruma AcmeAlpn; reuse odd-box cert store.
- WebSocket upgrades and h2c: cruma_proxy_lib supports; ensure we set protocols per backend hints.
- TCP passthrough routes for terminate_tls=false remote/hosted configs.
- Experimental runner: set `ODD_BOX_CRUMA_EXPERIMENTAL=1` to start the cruma hosting stack; use `ODD_BOX_CRUMA_PORT_OFFSET` (default 10000) to avoid port clashes while legacy listeners remain.
