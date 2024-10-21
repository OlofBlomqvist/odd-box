#!/bin/bash
set -e

# Install dependencies for Caddy
apt-get update
apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl

# Add Caddy GPG key and repository
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list

# Install Caddy
apt-get update
apt-get install -y caddy

# Install Rust nightly
rustup install nightly
rustup default nightly
