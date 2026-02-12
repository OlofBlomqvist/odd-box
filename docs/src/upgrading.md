
### Upgrading

If you are not using a package manager such as homebrew to manage your odd-box installation, you can either manually download new versions from the github release section or use the built in command for doing the same:
```odd-box --update```

When odd-box detects a package-managed installation (for example Homebrew/Nix/Snap/Cargo), `--update` is intentionally blocked and prints guidance for the package manager-specific upgrade command.
You can inspect what odd-box detected with:
```odd-box --install-source```

*Note: Should you have an older configuration file than V2, you can upgrade it automatically thru the ```odd-box --upgrade-config ./my-config-file.toml```.*
