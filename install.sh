#!/usr/bin/env sh
set -eu

repo="Cookie-HOO/ccusage-dsh"
install_dir="${CCUSAGE_DSH_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  *) echo "Unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch=x64 ;;
  arm64|aarch64) arch=arm64 ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

asset="ccusage-dsh-$os-$arch.tar.gz"
release_url="https://github.com/$repo/releases/latest/download"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

curl -fsSL "$release_url/$asset" -o "$tmp/$asset"
curl -fsSL "$release_url/SHA256SUMS" -o "$tmp/SHA256SUMS"
(
  cd "$tmp"
  if command -v sha256sum >/dev/null 2>&1; then
    grep "  $asset\$" SHA256SUMS | sha256sum -c -
  else
    expected="$(grep "  $asset\$" SHA256SUMS | awk '{print $1}')"
    actual="$(shasum -a 256 "$asset" | awk '{print $1}')"
    [ -n "$expected" ] && [ "$actual" = "$expected" ]
  fi
)

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$install_dir"
cp "$tmp/ccusage-dsh-$os-$arch/ccusage-dsh" "$install_dir/ccusage-dsh"
chmod +x "$install_dir/ccusage-dsh"

"$install_dir/ccusage-dsh" dsh --help >/dev/null
printf 'Installed ccusage-dsh to %s\n' "$install_dir/ccusage-dsh"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to PATH before running ccusage-dsh.\n' "$install_dir" ;;
esac
