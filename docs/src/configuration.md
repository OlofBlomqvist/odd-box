
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

4. Adding dir servers: for serving static websites or files. Supports directory indexing and markdown rendering.
    ```toml
    [[dir_server]]
    host_name = "dir.localtest.me"
    dir = "$cfg_dir"
    enable_directory_browsing = true
    render_markdown = true
    ```
