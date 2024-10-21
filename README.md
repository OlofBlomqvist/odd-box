[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml)

## ODD-BOX

A simple, cross-platform reverse proxy server tailored for local development and tinkering. Think of it as a lightweight (and more streamlined) alternative to something like IIS, but with a key difference: configuration is primarily done declaratively through structured files, rather than a graphical user interface.

It allows you to configure a list of processes to run and host them behind their own custom hostnames. Self-signed certificates for HTTPS are automatically generated when accessing a site thru the terminating proxy service the first time (cached in .odd-box-cache dir). As with most reverse-proxy servers, odd-box also supports targetting remote backend servers.

As configuration is done thru basic files (toml format) which are easy to share, it's very easy to reproduce a particular setup.

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases).

You can also build it yourself, or install it using brew, cargo, nix or devbox; see the installation section for guidance.

### Features

- Cross platform (win/lin/osx)
- Easy to configure (toml files)
- Keep a list of specified binaries running
- Serve local directories (for static sites)
- Uses PORT environment variable for routing
- Allows for setting proc specific and global env vars
- Remote target proxying
- Terminating proxy that supports both HTTP/1.1 & HTTP2
- TCP tunnelling for HTTP/1
- TCP tunnelling for HTTPS/1 & HTTP2 via SNI sniffing
- TCP tunnelling for HTTP/2 over HTTP/1 (h2c upgrade)
- H2C via terminating proxy 
- Automatic self-signed certs for all hosted processes
- Basic round-robin loadbalancing for remote targets
- Terminating proxy supports automaticly generating lets-encrypt certificates

 
### Performance

While the goal of this project is not to provide a state-of-the-art level performing proxy server for production environments, but rather a tool for simplifying local development scenarios, we do **try to** ~~keep performance in mind~~ **be blazingly fast** :-) Seriously though, performance is actually pretty good but it is not a priority (yet).

### Terminal User Interface

The TUI is fairly simple basic; it provides an easy way to see which sites are running, the log outputs and all currently active connections. It is possible to opt-out of TUI mode by supplying the argument: "--tui=false" when starting odd-box. 

### API

There is a basic administration API that can be enabled by adding "admin_api_port = n" to the configuration file. At some point a web-interface might be added for controlling odd-box thru this API..

### Screenshot(s)

**odd-box v0.1.2:**
![Screenshot of oddbox v0.1.2](/screenshot.jpg)


## Installation

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases),
but there are more ways to install odd-box if you so wish :-)


- [Homebrew](https://brew.sh/) 

    Recommended for Mac users. Brew lets you easily install odd-box globally and use brew for managing updates; it also works on Linux and Windows (wsl2).

```zsh
brew tap OlofBlomqvist/repo
brew install oddbox
```

- [Cargo install](https://doc.rust-lang.org/cargo/getting-started/installation.html)
```bash
cargo install odd-box
```

- [Devbox](https://www.jetify.com/devbox)
```json
{
  "name": "example devbox config",
  "packages": [
    // select rev of whichever version you need, this is for v0.1.2
    "github:OlofBlomqvist/odd-box?rev=043fe0abd9da1d4a1e0fa0bfcc300c71971e26ce"
    // ...
  ],
  // ...
}
```
- [Nix build](https://nix.dev/manual/nix/2.18/command-ref/nix-build)
```bash
 nix build github:OlofBlomqvist/odd-box
```
- [Nix Flake](https://nixos.wiki/wiki/Flakes)
```nix
{
  description = "example flake with oddbox";
  inputs = {
    ... # select rev of whichever version you need, this is for v0.1.2
    oddbox.url = "github:OlofBlomqvist/odd-box?rev=043fe0abd9da1d4a1e0fa0bfcc300c71971e26ce";
  };

  ...
}
```

### Workflow tips

If you are hosting a local project that you are currently working on, and want to do a rebuild without having to manually start and stop your site - you may want to consider having a pre-build step that does it for you:

You can enable or disable all sites or specific ones using the http://localhost:port/START and http://localhost:port/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site. Sites start automatically again on the next request. The same can be acomplished thru the admin-api if you enable it.

### DNS

As all the routing is based on hostnames, your client machine(s) must of course be able to resolve those names correctly to the proxy server IP. If you are working on a local machine this can be configured either by adding entries to your host file pointing each domain to 127.0.0.1 or by using something like [localtest.me](http://localtest.me/).me, eg. my-first-site.localtest.me when configuring hosted sites. More advanced users might use their own DNS servers to set up these domains however they like.


### Security tips

Since odd-box spawns your defined binaries, you should be careful not to run odd-box in elevated/admin mode. To be safe, use a non-restricted port so that you do not need root access or follow the section for your OS below!


## OS Specific guidance

#### MacOS:

MacOS does not require super-user access when binding to 0.0.0.0 / ::1 on ports 1-1024. The easiest way to get started is to **just set the ip and tls_ip to bind to 0.0.0.0:80 and 0.0.0.0:443**.

*Should you want to bind specifically to 127.0.0.1; do not run this application using sudo. Instead configure a redirect to a non-restricted port such as 8080, and configure odd-box to use that (port=8080):*

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

## Configuration (V2)

To configure the `odd-box` proxy server, create a configuration file following the format provided in the [example configuration file](https://github.com/OlofBlomqvist/odd-box/blob/main/odd-box-example-config.toml). The configuration file uses the TOML format and is designed to define both remote targets (sites that `odd-box` proxies traffic to) and hosted processes (sites or services managed directly by `odd-box`).

Running odd-box without any arguments it will first try to read from odd-box.toml, then Config.toml. You can supply a custom config path using: ./odd-box "/tmp/my-file.toml"

It is also possible to update the active configuration at runtime thru an administration API which fully documents the possible settings.

### Configuration Variables

| Variable    | Description                      |
|-------------|----------------------------------|
| $root_dir   | Resolves to whatever you set it to in the global configuration section. |
| $cfg_dir    | Resolves to the directory which the configuration toml file was read from. |
| $port       | Resolves to whatever port has been specified in the configuration. Only used for hosted processes. |

> Tip: if you are editing the confguration file using vs-code you should try the "even better toml" extension, which will provide you not only with syntax highlighting but also intellisense based on the configuration files '#:schema ...' tag.

### Basic Configuration Structure

There are more options than the ones shown here; these are the most commonly used ones. See the example configuration or schema.json file for a list of all possible options.

1. **Global Settings:** Set the global properties like `http_port`, `tls_port`, `ip`, `log_level`, `port_range_start`, and `env_vars`. These settings control the overall behavior of `odd-box`.

   ```toml
   #:schema https://raw.githubusercontent.com/OlofBlomqvist/odd-box/main/odd-box-schema-v2.json
   version = "V2"
   http_port = 8080
   tls_port = 4343
   ip = "127.0.0.1"
   log_level = "Trace"
   port_range_start = 4242
   env_vars = []
   lets_encrypt_account_email = "example@example.com"
   ``` 
   - ``version``: Must be "V2"
   - ``http_port``: TCP Port for the server to use. Defaults to 8080 if not specified.
   - ``tls_port``: TCP Port for the server to buse. Defaults to 4343 if not specified.
   - ``ip``: IP Address for the server to use. Defaults to 127.0.0.1 if not specified.
   - ``log_level``: info/warn/err/debug/trace - defaults to info of not specified.
   - ``port_range_start``: Must be specified - used for automatically assign the PORT env var to hosted sites (if not set explicity for a site).
   - ``env_vars``: List of environment variables that all hosted processes should have.
   - ``lets_encrypt_account_email``: (Optional) Set email to use if you wish to use lets-encrypt.

2. Adding Remote Targets: Define remote targets to forward traffic to external servers. Each remote_target requires a host_name (the incoming domain) and a list of backends (the target servers). To add a new remote site:
    ```toml
    [[remote_target]]
    host_name = "example.com"
    backends = [
        # hints are optional and used for specifying if server requires for example H2C.
        { https = true, address="example.com", port=443, hints = [] }
    ]
    ```
    - ``host_name``: The incoming domain that odd-box will listen for.
    - ``backends``: A list of backend servers to forward traffic to. The https property specifies if TLS is used.

3. Adding Hosted Processes: Define hosted processes that odd-box will manage. These are services that odd-box can start, stop, and restart. Each hosted_process requires a ``host_name``, ``dir``, ``bin``, and ``args``:
    ```toml
    [[hosted_process]]
    host_name = "myapp.local"
    dir = "/home/kalle/" # variables like $root_dir or $config_dir are allowed here
    bin = "/usr/bin/python3" # variables like $root_dir or $config_dir are allowed here
    args = ["-m", "http.server", "$port"] # variables like $port, $root_dir & $config_dir are allowed here
    auto_start = true 
    hints = ["NOH2","H2C","H2"] 
    https = false 
    env_vars = [
        { key = "some-environment-variable", value = "example-value" }, 
    ]
    ```
    - ``host_name``: The incoming domain that odd-box will listen for.
    - ``dir``: Directory where the process will be executed.
    - ``bin``: Executable path to a binary file (absolute, relative to dir, or in pwd)
    - ``args``: Arguments to pass to the binary.
    - ``auto_start``: (Optional) Set to true to automatically start the process with odd-box.
    - ``hints``: (Optional) Not normally needed but can be set to specify that a server requires for example H2C.
    - ``https``: (Optional) Set to true if the process uses HTTP (TLS)
    - ``enable_lets_encrypt``: (Optional) Set to true to enable lets-encrypt to be used for this site.




#### Getting Started

To get started quickly, simply copy the [minimal example configuration file](https://github.com/OlofBlomqvist/odd-box/blob/main/odd-box-example-config-minimal.toml), modify the relevant sections to add your remote targets or hosted processes, and run odd-box with your configuration file.


## Upgrading odd-box

If you are not using a package manager such as homebrew to manage your odd-box installation, you can either manually download new versions from the github release section or use the built in command for doing the same:
```odd-box --update```

*Note: Should you have an older configuration file than V2, you can upgrade it automatically thru the ```odd-box --upgrade-config ./my-config-file.toml```.*
