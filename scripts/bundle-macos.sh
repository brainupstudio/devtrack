#!/bin/sh
# Builds DevTrack.app.
#
#   scripts/bundle-macos.sh                       for this Mac's processor
#   scripts/bundle-macos.sh --universal           one app for Apple silicon and Intel
#   scripts/bundle-macos.sh --universal --dmg     also dist/DevTrack-<version>-macOS.dmg
#   scripts/bundle-macos.sh --install             also copy it to /Applications
#
# Signing: ad hoc by default, so the app runs on the Mac that built it and, on other Macs,
# opens after right-click › Open. With a Developer ID certificate in the keychain, set
# DEVTRACK_SIGN_ID="Developer ID Application: …" to sign it properly (notarization is a
# separate step: see RELEASE.md). Any other DEVTRACK_SIGN_ID (a name or SHA-1 hash) signs with
# that certificate as is, which is how the release CI keeps one identity across versions.
set -eu

cd "$(dirname "$0")/.."
NAME="DevTrack"
BUNDLE_ID="studio.brainup.devtrack"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
BUILD="$(git rev-list --count HEAD 2>/dev/null || echo 1)"
OUT="target/release/bundle"
APP="$OUT/$NAME.app"
SIGN_ID="${DEVTRACK_SIGN_ID:--}"
# offline on a developer Mac (dependencies already fetched); online in CI
OFFLINE="--offline"
[ "${CI:-}" = "true" ] && OFFLINE=""

UNIVERSAL=0; DMG=0; INSTALL=0
for a in "$@"; do
    case "$a" in
        --universal) UNIVERSAL=1 ;;
        --dmg) DMG=1 ;;
        --install) INSTALL=1 ;;
        *) echo "unknown option: $a" >&2; exit 2 ;;
    esac
done

# On Apple silicon the default toolchain may be an Intel one (its apps run translated): prefer a
# native toolchain from rustup when there is one. Release builds use their own folder, so the
# everyday build cache is left alone.
NATIVE="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
if [ -x "$NATIVE/cargo" ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = "1" ]; then
    PATH="$NATIVE:$PATH"
fi
export CARGO_TARGET_DIR="target/bundle-build"

if [ "$UNIVERSAL" = "1" ]; then
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
        MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release $OFFLINE --target "$t"
    done
    BIN="$CARGO_TARGET_DIR/universal/devtrack"
    mkdir -p "$(dirname "$BIN")"
    lipo -create -output "$BIN" "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/devtrack" "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/devtrack"
else
    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release $OFFLINE
    BIN="$CARGO_TARGET_DIR/release/devtrack"
fi

# a fresh bundle every time: move the old one aside inside target, never delete in place
mkdir -p "$OUT"
if [ -e "$APP" ]; then
    mv "$APP" "$OUT/.previous-$$.app"
fi
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/devtrack"
cp assets/icons/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>devtrack</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$BUILD</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleLocalizations</key><array><string>en</string><string>it</string></array>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>DevTrack $VERSION</string>
</dict>
</plist>
EOF

if [ "$SIGN_ID" = "-" ]; then
    codesign --force --sign - "$APP" >/dev/null
elif [ "${SIGN_ID#Developer ID}" = "$SIGN_ID" ]; then
    # a self-made certificate (the release CI uses one): it keeps the app's identity the same
    # from one version to the next, so macOS remembers "Always Allow" for the keychain
    codesign --force --sign "$SIGN_ID" "$APP"
else
    codesign --force --options runtime --timestamp --sign "$SIGN_ID" "$APP"
fi
codesign --verify --strict "$APP"
echo "built $APP ($VERSION, build $BUILD, $(lipo -archs "$APP/Contents/MacOS/devtrack"))"

if [ "$DMG" = "1" ]; then
    mkdir -p dist
    STAGE="$OUT/dmg-$$"
    mkdir -p "$STAGE"
    cp -R "$APP" "$STAGE/"
    ln -s /Applications "$STAGE/Applications"
    SUFFIX="macOS"
    [ "$UNIVERSAL" = "1" ] || SUFFIX="macOS-$(uname -m)"
    DMG_PATH="dist/$NAME-$VERSION-$SUFFIX.dmg"
    [ -e "$DMG_PATH" ] && mv "$DMG_PATH" "$OUT/.previous-$$.dmg"
    hdiutil create -volname "$NAME $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG_PATH" >/dev/null
    [ "$SIGN_ID" = "-" ] || codesign --force --sign "$SIGN_ID" "$DMG_PATH"
    echo "built $DMG_PATH ($(du -h "$DMG_PATH" | cut -f1))"
fi

if [ "$INSTALL" = "1" ]; then
    DEST="/Applications/$NAME.app"
    if [ -e "$DEST" ]; then
        mv "$DEST" "$OUT/.replaced-$$.app"
    fi
    cp -R "$APP" "$DEST"
    echo "installed $DEST"
fi
