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

pack:
  #!/usr/bin/env bash
  set -euo pipefail
  [ -d "demo-extension/dist" ] && rm -rf demo-extension/dist
  mkdir -p demo-extension/dist
  wasm-pack build --no-pack --no-typescript --target web demo-extension/popup
  wasm-pack build --no-pack --no-typescript --target web demo-extension/background
  wasm-pack build --no-pack --no-typescript --target web demo-extension/content
  
  cd ./demo-extension

  cp -r ./popup/pkg/* ./dist
  cp -r ./popup/assets ./dist
  rm -r ./popup/pkg
  cp ./background/pkg/background_bg.wasm ./dist
  cp ./background/pkg/background.js ./dist
  rm -r ./background/pkg
  cp ./content/pkg/content_bg.wasm ./dist
  cp ./content/pkg/content.js ./dist
  rm -r ./content/pkg

  cp ./manifest.json ./dist/manifest.json
  cp ./index.html ./dist/index.html
  cp ./index.js ./dist/index.js

pack-ext:
  #!/usr/bin/env bash
  set -euo pipefail
  just pack
  cargo run -p dx-ext watch