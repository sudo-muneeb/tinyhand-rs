#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release
mkdir -p dist
stage="dist/.deb-stage"
rm -rf "$stage"
mkdir -p "$stage/DEBIAN" "$stage/usr/bin" "$stage/usr/share/tinyhand-rs" \
  "$stage/usr/share/applications" "$stage/usr/share/icons/hicolor/scalable/apps" \
  "$stage/usr/share/doc/tinyhand-rs"
cp target/release/tinyhand-rs "$stage/usr/bin/"
cp -R assets "$stage/usr/share/tinyhand-rs/"
cp tinyhand-rs.desktop "$stage/usr/share/applications/"
cp assets/icons/tinyhand-rs.svg "$stage/usr/share/icons/hicolor/scalable/apps/"
cp README.md NOTICE.md LICENSE BUILD_REPORT.md "$stage/usr/share/doc/tinyhand-rs/"
cat > "$stage/DEBIAN/control" <<'EOF'
Package: tinyhand-rs
Version: 0.1.0
Section: utils
Priority: optional
Architecture: amd64
Maintainer: TinyHand for Linux contributors
Depends: libc6 (>= 2.34), libx11-6, libxext6, libxrender1, libxinerama1, libcairo2, libasound2
Description: Native X11 presentation pointer with fading ink
 36 hand styles and a laser dot, tap sounds, drawing and global shortcuts.
EOF
dpkg-deb --build --root-owner-group "$stage" dist/tinyhand-rs_0.1.0_amd64.deb
portable="dist/tinyhand-rs-0.1.0-linux-x86_64"
rm -rf "$portable"
mkdir -p "$portable"
cp target/release/tinyhand-rs README.md NOTICE.md LICENSE BUILD_REPORT.md tinyhand-rs.desktop "$portable/"
cp -R assets "$portable/"
tar -C dist -czf dist/tinyhand-rs-0.1.0-linux-x86_64.tar.gz "$(basename "$portable")"
tar --exclude=target --exclude=dist -czf dist/tinyhand-rs-0.1.0-source.tar.gz \
  --transform='s,^,tinyhand-rs-0.1.0/,' .
rm -rf "$stage" "$portable"
