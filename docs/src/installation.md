## Installation

Pre-built binaries are available in the [release section](https://github.com/OlofBlomqvist/odd-box/releases),
but there are more ways to install odd-box if you so wish :-)


- [Homebrew](https://brew.sh/) 

    Recommended for Mac users. Brew lets you easily install odd-box globally and use brew for managing updates; it also works on Linux and Windows (wsl2).

```zsh
brew tap OlofBlomqvist/repo
brew install odd-box
```

- [Cargo install](https://doc.rust-lang.org/cargo/getting-started/installation.html)
```bash
cargo install odd-box
```

- [Devbox](https://www.jetify.com/devbox)
```json
{
  "name": "example devbox config",
  "packages": [
    // select rev of whichever version you need, this is for v0.1.2
    "github:OlofBlomqvist/odd-box?rev=043fe0abd9da1d4a1e0fa0bfcc300c71971e26ce"
    // ...
  ],
  // ...
}
```
- [Nix build](https://nix.dev/manual/nix/2.18/command-ref/nix-build)
```bash
 nix build github:OlofBlomqvist/odd-box
```
- [Nix Flake](https://nixos.wiki/wiki/Flakes)
```nix
{
  description = "example flake with oddbox";
  inputs = {
    ... # select rev of whichever version you need, this is for v0.1.2
    oddbox.url = "github:OlofBlomqvist/odd-box?rev=043fe0abd9da1d4a1e0fa0bfcc300c71971e26ce";
  };

  ...
}
```
