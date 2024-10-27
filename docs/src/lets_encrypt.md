# Let’s Encrypt

Lets encrypt support works by creating a virtual file that can be used for proving ownership of a domain name. No support exists for the DNS auth mode.

You will need to have odd-box running on a server with a public IP and a DNS record pointing to it. Port 80 and 443 are both used and need to be open.

Certificates are automatically renewed when they have less than 30 days left of validity.