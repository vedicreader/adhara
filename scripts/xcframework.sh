#!/bin/sh
# Build swift/AdharaCore.xcframework: libadhara.a for Apple silicon and Intel Macs, with SQLite built in.
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=13.0
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$t" >/dev/null
  cargo build --release --lib --features static --target "$t"
done
mkdir -p target/universal target/headers
lipo -create target/aarch64-apple-darwin/release/libadhara.a target/x86_64-apple-darwin/release/libadhara.a -output target/universal/libadhara.a
cp include/adhara.h include/sqlite3.h target/headers/
printf 'module AdharaCore {\n  header "adhara.h"\n  link "c++"\n  export *\n}\n' > target/headers/module.modulemap
rm -rf swift/AdharaCore.xcframework
xcodebuild -create-xcframework -library target/universal/libadhara.a -headers target/headers -output swift/AdharaCore.xcframework
