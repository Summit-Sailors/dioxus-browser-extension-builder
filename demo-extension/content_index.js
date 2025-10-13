(async () => {
  try {
    const src = chrome.runtime.getURL("content.js");
    const wasmPath = chrome.runtime.getURL("content_bg.wasm");
    const wasmModule = await import(src);
    if (!wasmModule.default) throw new Error("WASM entry point not found!");
    await wasmModule.default({ module_or_path: wasmPath });
    wasmModule.main();
  } catch (err) {
    console.error("Failed to initialize WASM module:", err);
  }
})();
