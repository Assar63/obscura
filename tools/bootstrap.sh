#!/usr/bin/env bash
# Sets up this machine to build OBSCura: the native toolchain (`dev`) and/or
# the snap build (`snap`: snapcraft, LXD and the firewall rules LXD needs
# next to Docker). Safe to re-run: it only changes what's missing, and uses
# sudo for system-wide changes. `--check` reports without changing anything.
#
# Usage: tools/bootstrap.sh [--check] [dev|snap|all]   (default: all)

set -euo pipefail

check_only=false
target=all
for arg in "$@"; do
  case "$arg" in
    --check) check_only=true ;;
    dev | snap | all) target=$arg ;;
    -h | --help)
      sed -n '2,7s/^# \{0,1\}//p' "$0"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg (try --help)" >&2
      exit 2
      ;;
  esac
done

# Same packages as CI (.github/workflows/ci.yml) and the README, plus the
# compilers the snap build installs (clang/libclang for bindgen).
APT_PACKAGES=(
  build-essential curl wget file pkg-config clang libclang-dev
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev
  libayatana-appindicator3-dev librsvg2-dev libasound2-dev
)
RUST_MIN=1.77.2

problems=0
notes=()

if [ -t 1 ]; then
  green=$'\033[32m' yellow=$'\033[33m' cyan=$'\033[36m' reset=$'\033[0m'
else
  green='' yellow='' cyan='' reset=''
fi

ok() { printf '  %sok%s     %s\n' "$green" "$reset" "$*"; }
todo() {
  printf '  %stodo%s   %s\n' "$yellow" "$reset" "$*"
  problems=$((problems + 1))
}
# fix "description" command...: runs the command, or only reports it with --check.
fix() {
  local what=$1
  shift
  if $check_only; then
    todo "$what"
    return 0
  fi
  printf '  %s...%s    %s\n' "$cyan" "$reset" "$what"
  "$@"
  printf '  %sfixed%s  %s\n' "$green" "$reset" "$what"
}
have() { command -v "$1" >/dev/null 2>&1; }
# version_ge A B: A >= B
version_ge() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -1)" = "$2" ]; }

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/snap/bin:$PATH"

check_dev() {
  echo "Native build (dev):"

  if have dpkg-query && have apt-get; then
    local missing=() p
    for p in "${APT_PACKAGES[@]}"; do
      dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q "ok installed" || missing+=("$p")
    done
    if ((${#missing[@]})); then
      fix "install system packages: ${missing[*]}" \
        sudo apt-get install -y --no-install-recommends "${missing[@]}"
    else
      ok "system packages"
    fi
  else
    todo "not an apt-based system: install the Tauri Linux prerequisites by hand" \
      "(https://v2.tauri.app/start/prerequisites/#linux)"
  fi

  local rust
  rust=$(rustc --version 2>/dev/null | cut -d' ' -f2 || true)
  if [ -n "$rust" ] && version_ge "$rust" "$RUST_MIN"; then
    ok "Rust $rust"
  elif have rustup; then
    fix "update Rust ($rust is older than $RUST_MIN)" rustup update stable
  else
    fix "install Rust with rustup (in ~/.cargo and ~/.rustup)" sh -c \
      "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c rustfmt,clippy"
  fi

  # Vite needs Node 20.19+ or 22.12+.
  local node
  node=$(node --version 2>/dev/null | tr -d v || true)
  if [ -n "$node" ] && { version_ge "$node" 22.12 ||
    { version_ge "$node" 20.19 && ! version_ge "$node" 21; }; }; then
    ok "Node $node"
  elif have snap; then
    fix "install Node 22 (snap)${node:+, replacing $node on PATH}" \
      sudo snap install node --classic --channel=22
  else
    todo "install Node 20.19+ or 22.12+${node:+ (found $node)}"
  fi

  if have pnpm; then
    ok "pnpm $(pnpm --version)"
  elif have npm; then
    fix "install pnpm 10 (in ~/.local)" npm install -g --prefix "$HOME/.local" pnpm@10
  else
    todo "install pnpm 10 (needs Node first)"
  fi
}

check_snap() {
  echo "Snap build (snap):"

  if ! have snap; then
    todo "snapd is not installed"
    return 0
  fi

  if have snapcraft; then
    ok "snapcraft $(snapcraft --version 2>/dev/null | cut -d' ' -f2)"
  else
    fix "install snapcraft" sudo snap install snapcraft --classic
  fi

  if snap list lxd >/dev/null 2>&1; then
    ok "LXD"
  else
    fix "install LXD" sudo snap install lxd
  fi

  if id -nG "$USER" | grep -qw lxd; then
    ok "$USER is in the lxd group"
  else
    fix "add $USER to the lxd group" sudo usermod -aG lxd "$USER"
  fi
  # Group changes only reach new logins; sg picks them up straight away.
  if ! id -nG | grep -qw lxd; then
    notes+=("This shell isn't in the lxd group yet: build with" \
      "  sg lxd -c \"snapcraft pack --use-lxd\"" \
      "or log out and back in.")
  fi

  if ip link show lxdbr0 >/dev/null 2>&1; then
    ok "LXD initialised (lxdbr0)"
  elif snap list lxd >/dev/null 2>&1; then
    fix "initialise LXD" sudo lxd init --auto
  else
    todo "initialise LXD (after installing it): sudo lxd init --auto"
  fi

  check_firewall
}

# Docker sets the FORWARD policy to DROP, which cuts LXD containers off the
# network: snapcraft then fails with "A network related operation failed in
# a context of no network access". DOCKER-USER is Docker's chain for such
# exceptions. The rules don't survive a reboot, so re-run this before a
# snap build.
check_firewall() {
  ip link show lxdbr0 >/dev/null 2>&1 || return 0
  if ! have iptables; then
    ok "firewall (no iptables)"
    return 0
  fi

  if ! sudo -n true 2>/dev/null && ! [ -t 0 ]; then
    todo "firewall: can't check without sudo; run this in a terminal"
    return 0
  fi
  echo "  (reading the firewall needs root)"
  local policy
  policy=$(sudo iptables -S FORWARD | awk '$1 == "-P" { print $3 }')
  if [ "$policy" != DROP ]; then
    ok "firewall forwards LXD traffic (FORWARD policy $policy)"
    return 0
  fi
  if ! sudo iptables -n -L DOCKER-USER >/dev/null 2>&1; then
    todo "the FORWARD policy is DROP and there's no DOCKER-USER chain:" \
      "allow forwarding for lxdbr0 in your firewall"
    return 0
  fi

  local inbound=(-i lxdbr0 -j ACCEPT)
  local replies=(-o lxdbr0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT)
  if sudo iptables -C DOCKER-USER "${inbound[@]}" 2>/dev/null; then
    ok "firewall: LXD containers can reach the network"
  else
    fix "firewall: let LXD containers through Docker's DROP policy" \
      sudo iptables -I DOCKER-USER "${inbound[@]}"
  fi
  if sudo iptables -C DOCKER-USER "${replies[@]}" 2>/dev/null; then
    ok "firewall: replies reach LXD containers"
  else
    fix "firewall: let replies back to LXD containers" \
      sudo iptables -I DOCKER-USER "${replies[@]}"
  fi
}

case "$target" in
  dev) check_dev ;;
  snap) check_snap ;;
  all)
    check_dev
    echo
    check_snap
    ;;
esac

echo
for n in "${notes[@]}"; do echo "$n"; done
if $check_only && ((problems)); then
  echo "$problems thing(s) to fix: run without --check to fix them."
  exit 1
fi
if ((problems)); then
  echo "$problems thing(s) need fixing by hand (see above)."
  exit 1
fi
case "$target" in
  dev) echo "Ready. Build with: cd app && pnpm install && pnpm tauri build" ;;
  snap) echo "Ready. Build the snap with: snapcraft pack --use-lxd" ;;
  all) echo "Ready. Build with: cd app && pnpm install && pnpm tauri build," \
    "or the snap with: snapcraft pack --use-lxd" ;;
esac
