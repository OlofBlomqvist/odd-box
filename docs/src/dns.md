# DNS and Routing

Odd-box uses SNI-sniffing for TLS and otherwise peeks at HTTP Host headers in order to decide which target site to route traffic to. When using odd-box for local development you need to make sure that you can resolve the names you give each site.

Options include:

- **Host file entries:** Point domains to `127.0.0.1`.
- **localtest.me:** Use for testing (e.g., `my-site.localtest.me`).
- ***.localhost:** Use for testing (e.g., `my-site.localhost`).
- **Custom DNS:** For advanced users with DNS server control.


