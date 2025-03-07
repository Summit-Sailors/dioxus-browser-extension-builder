(async () => {
  try {
    const src = chrome.runtime.getURL("popup.js");
    const wasmPath = chrome.runtime.getURL("popup_bg.wasm");
    
    const contentMain = await import(src);

    if (!contentMain.default) throw new Error("WASM entry point not found!");
    
    await contentMain.default({ module_or_path: wasmPath });
    
  } catch (err) {
    console.error("Failed to initialize WASM module:", err);
  }
})();
