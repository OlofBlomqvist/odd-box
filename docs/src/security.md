# Security tips

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
