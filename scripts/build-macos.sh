#!/usr/bin/env bash
# Build a local macOS release without GitHub, tags, or pushing.
# Usage: ./scripts/build-macos.sh v0.4.13 [--windows-exe]
set -euo pipefail

tag="${1:-}"
if [[ ! "$tag" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Usage: $0 vX.Y.Z" >&2
  exit 2
fi
version="${tag#v}"
with_windows=false
if [[ "${2:-}" == "--windows-exe" ]]; then
  with_windows=true
elif [[ -n "${2:-}" ]]; then
  echo "Usage: $0 vX.Y.Z [--windows-exe]" >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script must run on macOS." >&2
  exit 1
fi

if ! git diff --quiet --exit-code || ! git diff --cached --quiet --exit-code; then
  echo "Note: building from a dirty worktree; only Cargo.toml/Cargo.lock may change for the requested version."
fi

if [[ "$with_windows" == true ]]; then
  # cargo-zigbuild does not implement `--version`; checking its executable is
  # reliable and avoids treating a valid install as missing.
  if ! command -v zig >/dev/null || ! command -v cargo-zigbuild >/dev/null; then
    cat >&2 <<'EOF'
Windows .exe cross-build needs Zig and cargo-zigbuild. Install once, then rerun:
  brew install zig
  cargo install cargo-zigbuild --locked
  rustup target add x86_64-pc-windows-gnu
EOF
    exit 1
  fi
fi

# The package stanza is deliberately at the top of Cargo.toml. BSD sed is used
# because it ships with every supported macOS release.
echo "==> Building cloudshell v${version} for macOS"
sed -i '' "3s/^version = .*/version = \"${version}\"/" Cargo.toml

cargo build --release
cargo test

reported="$(cargo run --quiet --release -- --version)"
if [[ "$reported" != "cloudshell $version" ]]; then
  echo "Version check failed: $reported" >&2
  exit 1
fi

case "$(uname -m)" in
  arm64) arch="aarch64" ;;
  x86_64) arch="x86_64" ;;
  *) echo "Unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac

dist_dir="$repo_root/dist"
app="$dist_dir/cloudshell.app"
dmg="$dist_dir/cloudshell-v${version}-macos-${arch}.dmg"
contents="$app/Contents"

rm -rf "$app" "$dmg" "$dist_dir/cloudshell.iconset"
mkdir -p "$contents/MacOS" "$contents/Resources" "$dist_dir/cloudshell.iconset"
cp target/release/cloudshell "$contents/MacOS/cloudshell"

# Build a native icon bundle from the source PNG.
iconset="$dist_dir/cloudshell.iconset"
sips -z 16 16 assets/icon@512.png --out "$iconset/icon_16x16.png" >/dev/null
sips -z 32 32 assets/icon@512.png --out "$iconset/icon_16x16@2x.png" >/dev/null
sips -z 32 32 assets/icon@512.png --out "$iconset/icon_32x32.png" >/dev/null
sips -z 64 64 assets/icon@512.png --out "$iconset/icon_32x32@2x.png" >/dev/null
sips -z 128 128 assets/icon@512.png --out "$iconset/icon_128x128.png" >/dev/null
sips -z 256 256 assets/icon@512.png --out "$iconset/icon_128x128@2x.png" >/dev/null
sips -z 256 256 assets/icon@512.png --out "$iconset/icon_256x256.png" >/dev/null
sips -z 512 512 assets/icon@512.png --out "$iconset/icon_256x256@2x.png" >/dev/null
sips -z 512 512 assets/icon@512.png --out "$iconset/icon_512x512.png" >/dev/null
sips -z 1024 1024 assets/icon@512.png --out "$iconset/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$iconset" -o "$contents/Resources/cloudshell.icns"

sed "s/__VERSION__/${version}/g" assets/Info.plist > "$contents/Info.plist"

# Ad-hoc signing makes the local bundle launchable. Public distribution still
# needs a Developer ID certificate plus notarization.
codesign --force --deep --sign - "$app"
codesign --verify --verbose "$app"
hdiutil create -volname "cloudshell" -srcfolder "$app" -ov -format UDZO "$dmg" >/dev/null

echo "Created local app: $app"
echo "Created local installer: $dmg"

if [[ "$with_windows" == true ]]; then
  windows_target="x86_64-pc-windows-gnu"
  windows_exe="$dist_dir/cloudshell-v${version}-windows-x86_64.exe"
  echo "==> Cross-building cloudshell v${version} for Windows"

  # Zig opens the Rust objects concurrently during the final PE link. Give the
  # linker a reasonable descriptor budget when this script was launched by an
  # app or CI runner with macOS's low default soft limit.
  fd_soft="$(ulimit -Sn)"
  if [[ "$fd_soft" != "unlimited" ]] && (( fd_soft < 4096 )); then
    fd_hard="$(ulimit -Hn)"
    fd_target=4096
    if [[ "$fd_hard" != "unlimited" ]] && (( fd_hard < fd_target )); then
      fd_target="$fd_hard"
    fi
    if ! ulimit -Sn "$fd_target"; then
      echo "Warning: could not raise the open-file limit above $fd_soft." >&2
    fi
  fi

  # Thin LTO makes rustc hand Zig hundreds of individual dependency objects.
  # Zig 0.16 can then fail with ProcessFdQuotaExceeded. Keep Thin LTO for the
  # native macOS release, but disable it for this cross-link only.
  CARGO_PROFILE_RELEASE_LTO=off \
    cargo zigbuild --release --target "$windows_target"
  cp "target/$windows_target/release/cloudshell.exe" "$windows_exe"
  echo "Created Windows executable: $windows_exe"
fi

echo "Cargo.toml and Cargo.lock now contain $version; commit them if releasing."
