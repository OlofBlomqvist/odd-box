const SettingDescriptions = {
    "h2_hint": "Hints are suggestions from the server to the client on how to prioritize resources or manage network behavior.",
    "hostname": "Choose hostname for this site.",
    "hostname_frontend": "Incoming name for binding to (frontend).",
    "port": "Choose port for this site. Leave blank to use port from global settings.",
    "https_port": "Choose TLS port for this site. Leave blank to use port from global settings.",
    "directory": "Path to working dir of this site.",
    "binary": "Name of the binary file to run.",
    "https": "Use HTTPS.",
    "auto_start": "Enable this if you want this site to start automatically when oddbox starts.",
    "capture_subdomains": "Instead of only listening to yourdomain.com, you can capture subdomains which means this site will also respond to requests for *.yourdomain.com",
    "disable_tcp_tunnel": "This is mostly useful in case the target uses SNI sniffing/routing.",
    "forward_subdomains": "If you wish to use the subdomain from the request in forwarded requests: test.example.com -> internal.site vs test.example.com -> test.internal.site.",
    "log_format": "Choose format for logs.",
    "default_log_format": "Default format for logs.",
    "global_env_vars": "These are global environment variables - they will be set for all hosted processes.",
    "log_level": "Choose your preferred log level.",
    "args": "Arguments to the binary.",
    "env_vars": "These will be set on launch.",
    "root_dir": "Root directory for sites.",
    "default_http_port": "Default port for new sites.",
    "default_tls_port": "Default TLS port for new sites.",
    "default_auto_start": `Default value for configured sites. False means a site will not start automatically with odd-box, but it will still be automatically started on incoming requests to that site.`,
    "proxy_ip": "IP for proxy to listen to , can be ipv4/6.",
    "port_range_start": "Port range for automatic port assignment (the env var PORT will be set if you did not specify one manually for a process).",
    "use_alpn": "Allows alpn negotiation for http/1.0 and h2 on tls connections.",
    "site_type": "Choose the type of site you are adding.",
    "exclude_from_start_all": "Will exempt the site from the start/stop all sites feature.",
    "backends": "All backends for this site.",
    "remote_site_address": "Address of the remote server.",
    "enable_directory_browsing": "If enabled, the directory server will serve files from the root directory.",
    "enable_lets_encrypt": "If enabled, the directory server will automatically obtain and renew TLS certificates from lets-encrypt.",
    "lets_encrypt_account_email": "Email address for lets-encrypt account.",
}

export default SettingDescriptions;