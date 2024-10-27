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

From here, you can either open up the config file in your favorite editor, or just run odd-box and open your browser going to http://localhost:1234 where you can configure odd-box thru its web-interface.

## Documentation

For more in depth guidance on using odd-box, see the [documentation](https://odd-box.cruma.io).
