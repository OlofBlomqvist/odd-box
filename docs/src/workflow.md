### Workflow tips

If you are hosting a local project that you are currently working on, and want to do a rebuild without having to manually start and stop your site - you may want to consider having a pre-build step that does it for you:

You can enable or disable all sites or specific ones using the http://localhost:port/START and http://localhost:port/STOP endpoints, optionally using query parameter "?proc=my_site" to stop or start a specific site. Sites start automatically again on the next request. The same can be acomplished thru the admin-api.

### DNS

As all the routing is based on hostnames, your client machine(s) must of course be able to resolve those names correctly to the proxy server IP. If you are working on a local machine this can be configured either by adding entries to your host file pointing each domain to 127.0.0.1 or by using something like [localtest.me](http://localtest.me/).me or [test.localhost](http://test.localhost/), eg. my-first-site.localtest.me when configuring hosted sites. More advanced users might use their own DNS servers to set up these domains however they like.

### Configuration

1. Make use of the variable support when configuring sites! Here are some examples:


```toml
[[hosted_process]]
host_name = "py.localtest.me"
bin = "$cfg_dir/python" # <--- $cfg_dir here will expand to whichever directory your odd-box.toml configuration file lives
args = [
  "-m", 
  "http.server", 
  "$port" # <--- Here, the $port variable will be replaced by odd-box 
]
```

```toml
[[hosted_process]]
host_name = "nginx.localhost"
bin = "podman"
args = [
  "run", 
  "--replace", 
  "--quiet", 
  "-p$port:80", # <--- Here, the $port variable will be replaced by odd-box 
  "--name", 
  "odd-nginx",
  "nginx"
]
```

