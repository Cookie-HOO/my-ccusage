#!/usr/bin/env sh
set -eu

repo="Cookie-HOO/my-ccusage"
install_dir="${MY_CCUSAGE_INSTALL_DIR:-$HOME/.local/bin}"

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

asset="my-ccusage-$os-$arch.tar.gz"
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
cp "$tmp/my-ccusage-$os-$arch/my-ccusage" "$install_dir/my-ccusage"
chmod +x "$install_dir/my-ccusage"

"$install_dir/my-ccusage" dsh --help >/dev/null
"$install_dir/my-ccusage" workbuddy --help >/dev/null
printf 'Installed my-ccusage to %s\n' "$install_dir/my-ccusage"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to PATH before running my-ccusage.\n' "$install_dir" ;;
esac
