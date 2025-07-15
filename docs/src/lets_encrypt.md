# Let’s Encrypt

Odd-box supports automatically generating and renewing certificates using lets-encrypt - no certbot required.

You will need to have odd-box running on a server with a public IP and a DNS record pointing to it. Port 80 and 443 are both used and need to be open.

Certificates are automatically renewed when they have less than 30 days left of validity.

## Example Configuration
 
```toml
lets_encrypt_account_email = "some-email@example.com"
  
[[remote_target]]
host_name = "api.example.com"
enable_lets_encrypt = true # Enable LE
redirect_to_https = true
...

[[dir_server]]
host_name                 = "static.example.com"
dir                       = "/var/www/public" 
enable_lets_encrypt       = true # Enable LE
enable_directory_browsing = true
render_markdown           = true
redirect_to_https         = true
...

```toml
[[hosted_process]]
host_name              = "app.local"
enable_lets_encrypt    = true # Enable LE
...
```

### How it works

When a request is made to odd-box using HTTP(s) with a hostname where we have not yet generated a certificate, odd-box will complete the lets-encrypt challenge and generate a certificate which is then used to serve the site.

You can read more about the entire process here: https://letsencrypt.org/docs/challenge-types/#http-01-challenge