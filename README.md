[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/BuildAndTest.yml)

## ODD-BOX

A simple, cross-platform reverse proxy server tailored for local development and tinkering. Think of it as a lightweight (and more streamlined) alternative to something like IIS, but with a key difference: configuration is primarily done declaratively through structured files, rather than a graphical user interface.

It allows you to configure a list of processes to run and host them behind their own custom hostnames. Self-signed certificates for HTTPS are automatically generated when accessing a site thru the terminating proxy service the first time (cached in .odd-box-cache dir). As with most reverse-proxy servers, odd-box also supports targetting remote backend servers.

As configuration is done thru basic files (toml format) which are easy to share, it's very easy to reproduce a particular setup.

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases).

You can also build it yourself, or install it using brew, cargo, nix or devbox; see the installation section for guidance.

### Screenshot(s)

**odd-box v0.1.2:**
![Screenshot of oddbox v0.1.2](/screenshot.jpg)

**odd-box web-ui v0.1.8:**
![Screenshot of oddbox v0.1.8](/webui-screenshot.jpg)



## Getting Started

You can generate a basic "odd-box.toml" config file to get started:
```
odd-box --init 
```

The resulting file will look something like this:
```toml
#:schema https://raw.githubusercontent.com/OlofBlomqvist/odd-box/main/odd-box-schema-v3.0.json

# Global settings
version = "V3"
ip = "127.0.0.1" 
http_port = 8080        
tls_port = 4343        

# ==========================================================

# This serves the directory where this file is located on the dir.localtest.me domain.
[[dir_server]]
host_name = "dir.localtest.me"
dir = "$cfg_dir"

# This sets up a basic reverse proxy to lobste.rs which you can reach thru the lobsters.localtest.me domain
[[remote_target]] 
host_name = "lobsters.localtest.me" 
backends = [ 
    { address = "lobste.rs", port = 443, https = true }
]

# This will spin up a python http server in the root directory of the config file (where this file is) -
# you can reach it thru the py.localtest.me domain.
[[hosted_process]]
host_name = "py.localtest.me"
bin = "python"
auto_start = false
args = ["-m", "http.server", "$port"] 

# Example for running a docker container
[[hosted_process]]
host_name = "nginx.localhost"
bin = "podman"
args = [ 
    "run",
    "--replace",
    "--quiet", 
    "-p$port:80", # incoming $port is handled by odd-box
    "nginx" # <-- image name
]

# oh and you can ofc also host processes that dont actually listen to a port
# but you just want to keep running :)

```

From here, you can either open up the config file in your favorite editor, or just run odd-box and open your browser going to https://localhost:4343 where you can configure odd-box thru its web-interface.

## Documentation

For more in depth guidance on using odd-box, see the [documentation](https://odd-box.cruma.io).
