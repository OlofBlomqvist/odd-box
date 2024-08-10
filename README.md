[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml)

## ODD-BOX

A simple to use cross-platform **toy-level** reverse proxy server for local development and tinkering purposes.

It allows you to configure a list of processes to run and host them behind their own custom hostnames. Automatically generates (self-signed) certificates for HTTPS when accessing them the first time (cached in .odd-box-cache dir).

Uses the 'port' environment variable to assign a port for each site. If your process does not support using the port environment variable, you can pass custom arguments or variables for your process instead.

You can enable or disable all sites or specific ones using the http://localhost/START and http://localhost/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site. (Mostly only useful for pre-build scripts where you dont want to manually stop and start the proxy on each rebuild. Sites start automatically again on the next request). The same can be acomplished thru the admin-api if you enable it.

### Main Features & Goals

- Cross platform (win/lin/osx)
- Easy to configure
- Keep a list of specified binaries running
- Uses PORT environment variable for routing
- Allows for setting proc specific and global env vars
- Remote target proxying
- Terminating proxy that supports both HTTP/1.1 & HTTP2
- TCP tunnelling for HTTP/1
- TCP tunnelling for HTTPS/1 via SNI sniffing
- TCP tunnelling for HTTP/2 over HTTP/1 (h2c upgrade)
- H2C via terminating proxy 
- Automatic self-signed certs for all hosted processes

### Performance

While the goal of this project is not to provide a state-of-the-art level performing proxy server, but rather a tool for simplifying local development scenarios, we do try to keep performance ok:ish.. TCP tunnel mode supports 100k+ requests per second while the intercepting proxy mode allows for around 20k-50k requests per second in most cases. More specific measurements of different scenarios will be added here at some point.

### Terminal User Interface

The TUI is fairly simple basic; it provides an easy way to see which sites are running, the log outputs and all currently active connections.

### API

There is a basic administration API that can be enabled by adding "admin_api_port = n" to the configuration file. At some point a web-interface might be added for controlling odd-box thru this API..

**odd-box v0.0.9:**
![Screenshot of oddbox v0.0.9](/screenshot.png)

### Configuration

By default, running odd-box without any arguments it will first try to read from odd-box.toml, then Config.toml. You can supply a custom config path using: ./odd-box "/tmp/my-file.toml"

See the odd-box-example-config.toml file in this repository for details around how to configure oddbox.

*Due to the fact that we now support more than a few configuration options, it is also possible to update the active configuration at runtime thru an administration API which fully documents the possible settings.*

### Configuration Variables

| Variable   | Description                      |
|------------|----------------------------------|
| $root_dir  | Resolves to whatever you set it to in the global configuration section. |
| $cfg_dir   | Resolves to the directory which the configuration toml file was read from. |

### DNS

Since all the routing is based on hostnames, your client machine(s) must of course be able to resolve those names correctly to the proxy server IP. If you are working on a local machine this can be configured either by adding entries to your host file pointing each domain to 127.0.0.1 or by using something like [localtest.me](http://localtest.me/).me, eg. my-first-site.localtest.me when configuring hosted sites. More advanced users might use their own DNS servers to set up these domains however they like.



### Security tips

Since odd-box spawns your defined binaries, you should be careful not to run odd-box in elevated/admin mode. To be safe, use a non-restricted port or follow the section for your OS below!

#### MacOS:

Do not run this application using sudo. If you want to listen to port 80, configure a redirect to a non-restricted port such as 8080, and configure odd-box to use that (port=8080) instead.

```bash
rdr pass on lo0 inet proto tcp from 127.0.0.1 to 127.0.0.1 port 80 -> 127.0.0.1 port 8080
sudo pfctl -ef pf-rules.conf
```

#### Linux:

Do not run this application using sudo. Instead allow odd-box to listen to restricted ports directly.

```bash
sudo setcap CAP_NET_BIND_SERVICE=+eip /path/to/odd-box
# (alternatively you could set up a redirect in your fw, similar to the MacOS section)
```

#### Windows:

Do not run the application as admin (elevated mode), instead you can allow your own account to use restricted ports.

```powershell
netsh http add urlacl url=http://+:80/ user=DOMAIN\user
# (alternatively you could set up a redirect in your fw, similar to the MacOS section)
```
