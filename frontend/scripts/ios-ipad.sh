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
xcodegen generate
