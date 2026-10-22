#!/bin/sh
# Makes the generated iOS project iPad-only. Run after `tauri ios init`, which regenerates the project and drops this setting.
# TARGETED_DEVICE_FAMILY 2 = iPad, 1 = iPhone. Needs xcodegen (brew install xcodegen).
set -e
cd "$(dirname "$0")/../src-tauri/gen/apple"
python3 - <<'PY'
import re
path = "project.yml"
text = open(path).read()
if "TARGETED_DEVICE_FAMILY" not in text:
    text = text.replace("        ENABLE_BITCODE: false\n", "        ENABLE_BITCODE: false\n        # iPad only: 2 = iPad, 1 = iPhone\n        TARGETED_DEVICE_FAMILY: \"2\"\n", 1)
    open(path, "w").write(text)
PY
# Bundle ID for the iOS app. The shared ID is taken by another developer, and changing the shared identifier would move the
# desktop app's data folder, so only the iOS app gets its own: IOS_BUNDLE_ID, for example com.yourname.trainlink
if [ -n "$IOS_BUNDLE_ID" ]; then
  python3 - "$IOS_BUNDLE_ID" <<'PY'
import re, sys
path = "project.yml"
text = open(path).read()
# The app group's default ID (the iOS project's only app target), so every build uses it
text = re.sub(r"(\n      PRODUCT_BUNDLE_IDENTIFIER: )[^\n]+", lambda m: m.group(1) + sys.argv[1], text, count=1)
open(path, "w").write(text)
PY
fi
# Signing team for device builds: IOS_TEAM_ID is your Apple team ID (shown in Xcode's account settings), kept out of the repo
if [ -n "$IOS_TEAM_ID" ]; then
  python3 - "$IOS_TEAM_ID" <<'PY'
import sys
path = "project.yml"
text = open(path).read()
if "DEVELOPMENT_TEAM" not in text:
    text = text.replace("        ENABLE_BITCODE: false\n", "        ENABLE_BITCODE: false\n        DEVELOPMENT_TEAM: \"" + sys.argv[1] + "\"\n        CODE_SIGN_STYLE: Automatic\n", 1)
    open(path, "w").write(text)
PY
fi
xcodegen generate
