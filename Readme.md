[![BuildAndTest](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml/badge.svg)](https://github.com/OlofBlomqvist/odd-box/actions/workflows/rust.yml)

## ODD-BOX

A simple to use cross-platform **toy-level** reverse proxy server for local development and tinkering purposes.

It allows you to configure a list of processes to run and host them behind their own custom hostnames.

Uses the 'port' environment variable to assign a port for each site. If your process does not support using the port environment variable, you can pass custom arguments or variables for your process instead.

You can enable or disable all sites or specific ones using the http://localhost/START and http://localhost/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site.
(Mostly only useful for pre-build scripts where you dont want to manually stop and start the proxy on each rebuild. Sites start automatically again on the next request) 

By default, running odd-box without any arguments it will first try to read from odd-box.toml, then Config.toml.
You can supply a custom config path using: ./odd-box "/tmp/my-file.toml"

Configuration format:

```toml
# Global configuration
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
path = "$root_dir\\sites\\fun"
bin = "my_site.exe"
args = ["/woohoo"]
https = false # optional bool
log_format = "standard" # optional - overrides default_log_format if defined
env_vars = [
    { key = "site_specific_env_var", value = "hello"},
]

# Host a node based site
[[processes]]
host_name = "noodle.local"
path = "c:\\temp"
bin = "node"
args = ["app.js"]
env_vars = []

# Hosting an asp.net 4.8 based site behind IIS express 
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
