set shell := ["bash", "-uc"]

default:
  @just --choose --justfile {{justfile()}}

clear:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo clean
  rm */lock

sort-d:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo sort-derives

pack-ext:
  #!/usr/bin/env bash
  set -euo pipefail
  just pack
  cargo run -p extension-builder