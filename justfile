set shell := ["bash", "-uc"]

default:
  @just --choose --justfile {{justfile()}}

clear:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo clean
  rm *.lock

sort-d:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo sort-derives

ext-watch:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo run -p dioxus-browser-extension-builder watch
  
demo-server:
  #!/usr/bin/env bash
  set -euo pipefail
  dx serve --server -p server