# Installation Guide

`ripwire-rs` ships prebuilt binaries and native system packages for macOS (`aarch64`, `x86_64`), Linux (`x86_64`, `aarch64`), and Windows (`x86_64`), as well as source builds via Cargo.

---

## Homebrew (macOS & Linux)

```bash
brew install casonadams/tap/ripwire-rs
```

Upgrade to the latest release:

```bash
brew update && brew upgrade ripwire-rs
```

---

## APT Repository (Ubuntu & Debian — `amd64` & `arm64`)

Add the APT repository hosted on GitHub Pages and install `ripwire-rs`:

```bash
echo "deb [trusted=yes] https://casonadams.github.io/homebrew-tap/apt stable main" | sudo tee /etc/apt/sources.list.d/casonadams.list
sudo apt update && sudo apt install ripwire-rs
```

Or download a standalone `.deb` asset directly from [GitHub Releases](https://github.com/casonadams/ripwire-rs/releases/latest):

```bash
gh release download --repo casonadams/ripwire-rs --pattern "ripwire-rs_*_amd64.deb"
sudo dpkg -i ripwire-rs_*_amd64.deb
```

---

## DNF / RPM Repository (Fedora, RHEL & CentOS — `x86_64` & `aarch64`)

Add the RPM repository and install `ripwire-rs`:

```bash
sudo dnf config-manager --add-repo https://casonadams.github.io/homebrew-tap/rpm/casonadams.repo
sudo dnf install ripwire-rs
```

---

## Cargo & `cargo-binstall`

Install the prebuilt binary in ~1 second using [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall ripwire-rs
```

Or compile from crates.io source:

```bash
cargo install ripwire-rs
```

---

## From Source

```bash
git clone https://github.com/casonadams/ripwire-rs.git
cd ripwire-rs
make install
```

Builds the optimized release binary (`target/release/ripwire-rs`) and installs the manual page (`man ripwire-rs`).

---

## Post-Install: Agent Skills & MCP Server

After installing the `ripwire-rs` binary, optionally install the 17 embedded agent skills (`~/.agents/skills`) or register the stdio MCP server:

```bash
# Install 17 ripwire-rs agent skills
ripwire-rs --add-skills

# Verify installation and environment health
ripwire-rs . --doctor
```
