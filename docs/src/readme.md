# What is ODD-BOX?

**ODD-BOX** is a lightweight, cross-platform reverse proxy-server & web-server, designed specifically for developers who need to manage and test local or remote services seamlessly. Odd-box is built for speed and simplicity, allowing you to set up and manage services using easy-to-edit TOML files as well as providing a web-ui.

It can also function as an ordinary webserver, serving static files; or as a way to just keep some specific set of binaries running regardless of them being websites or not.

With `odd-box`, you can:

- Easily define and control local or remote services: Use declarative TOML configurations to specify which services to run and manage, each with its own hostname, environment variables, and settings.

- Host services with custom domain names: Each service can be mapped to a unique hostname, allowing local services to behave like they’re hosted in production.

- Seamlessly route traffic: Proxy traffic to both local and remote targets, supporting HTTP/1.1, HTTP2, and HTTPS.

- Serve static files or sites locally: Ideal for developers working on static sites or frontend projects that need quick, local hosting.

- Use a Terminal UI (TUI) or Web UI: Choose between a compact TUI for terminal-based monitoring and a dark-themed web UI for configuration management and live logs.

Whether you’re running multiple microservices on your local machine, testing secure connections, or need a streamlined way to spin up and shut down services for testing, `odd-box` can help you save time and reduce complexity.

## Key Features:

- Automatic Certificate Generation: Generate self-signed certificates for HTTPS connections or integrate with **Let’s Encrypt** for public-facing services.

- Load Balancing: Simple round-robin load balancing for remote targets.

- Flexible and Portable Configuration: All settings are in sharable TOML files, making it easy to replicate setups on other machines.

- Security-Oriented: Avoid the need for root privileges with OS-specific guides for setting up restricted ports.

- Configurable via API, WebUI or directly by editing your config file on disk.

If you're tired of manually configuring proxy servers or need a tool to streamline your local and remote service setup, give odd-box a try.
