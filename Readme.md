[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml)

## ODD-BOX

A simple to use cross-platform **toy-level** reverse proxy server for local development and tinkering purposes.

It allows you to configure a list of processes to run and host them behind their own custom hostnames.

Automatically generates (self-signed) certificates for HTTPS when accessing them the first time (cached in .odd-box-cache dir).

Uses the 'port' environment variable to assign a port for each site. If your process does not support using the port environment variable, you can pass custom arguments or variables for your process instead.

You can enable or disable all sites or specific ones using the http://localhost/START and http://localhost/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site.
(Mostly only useful for pre-build scripts where you dont want to manually stop and start the proxy on each rebuild. Sites start automatically again on the next request) 

By default, running odd-box without any arguments it will first try to read from odd-box.toml, then Config.toml.
You can supply a custom config path using: ./odd-box "/tmp/my-file.toml"

Configuration format:

```toml
# Global configuration
port = 80 # optional - 8080 by default
tls_port = 443 # optional - 4343 by default
root_dir = "c:\\temp" # optional - can be used as a variable in process paths
log_level = "info" # trace,debug,info,warn,error
port_range_start = 4200 # will start serving on 4200 and up 1 for each site
default_log_format = "dotnet" # optional: "dotnet" or "standard"
env_vars = [
     # can be overridden by site specific ones
     { key = "env_var_for_all_sites" , value = "nice" }, 
]

# Normal host
[[processes]]
host_name = "fun.local"
path = "$root_dir/sites/fun"
bin = "my_server_binary_file_name" # you can leave out .exe from the filename also on windows for xplat comp
args = ["/woohoo","/test:$cfg_dir","/test2: $root_dir"]
https = false # optional bool - defaults to false
log_format = "standard" # optional - overrides default_log_format if defined
env_vars = [
    { key = "site_specific_env_var", value = "hello"},
    { key = "port", value = "9999"},
]

# Host a node based site
[[processes]]
host_name = "noodle.local"
path = "$cfg_dir/my_site"
bin = "node"
args = ["app.js"]
env_vars = []

# Hosting an asp.net 4.8 based site behind IIS express  (windows only)
[[processes]]
host_name = "cool-site.local"
path = "C:\\Program Files\\IIS Express"
bin = "iisexpress.exe"
args = [ 
    "/path:c:\\temp\\cool-site", 
    "/port:12345",
    "/hostname:127.0.0.1",
    "/trace:error"]
env_vars = [
    # since port was configured in an arg, we need to also specify it here
    # to override the otherwise automatic port configuration
    { key = "port", value = "12345"},
]


```

### Configuration Variables

| Variable   | Description                      |
|------------|----------------------------------|
| $root_dir  | Resolves to whatever you set it to in the global configuration section. |
| $cfg_dir   | Resolves to the directory which the configuration toml file was read from. |



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
