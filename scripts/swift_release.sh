#!/bin/sh
# Build the release xcframework zip and write its url and checksum into Package.swift.
# Run it before ship-release, commit Package.swift, and upload target/AdharaCore.xcframework.zip to the release.
set -eu
cd "$(dirname "$0")/.."
scripts/xcframework.sh
zip=target/AdharaCore.xcframework.zip
rm -f "$zip"
ditto -c -k --sequesterRsrc --keepParent swift/AdharaCore.xcframework "$zip"
sum=$(swift package compute-checksum "$zip")
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
sed -i '' -e "s|releases/download/v[^/]*/|releases/download/v$ver/|" -e "s|checksum: \"[0-9a-f]*\"|checksum: \"$sum\"|" Package.swift
grep -q "releases/download/v$ver/" Package.swift && grep -q "$sum" Package.swift || { echo "Package.swift was not updated" >&2; exit 1; }
echo "v$ver $sum $zip"
