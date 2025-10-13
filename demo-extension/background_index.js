(async () => {
  try {
    importScripts(chrome.runtime.getURL("background.js"));
    const wasmPath = chrome.runtime.getURL("background_bg.wasm");
    await wasm_bindgen(wasmPath);
    wasm_bindgen.main();
  } catch (err) {
    console.error("Failed to initialize WASM module:", err);
  }
})();
