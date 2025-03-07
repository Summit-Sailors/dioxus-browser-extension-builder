(async () => {
  try {
    const src = chrome.runtime.getURL("content.js");
    const wasmPath = chrome.runtime.getURL("content_bg.wasm");

    const contentMain = await import(src);

    if (!contentMain.default) throw new Error("WASM entry point not found!");
    await contentMain.default({ module_or_path: wasmPath });

    // attaching extract function to window
    window.contentMain = contentMain;
  } catch (err) {
    console.error("Failed to initialize WASM module:", err);
  }
})();

chrome.runtime.onMessage.addListener(async (request, sender, sendResponse) => {
  if (request.action === "extractContent") {
    try {
      if (window.contentMain && typeof window.contentMain.extract === "function") {
        const extractedContent = await window.contentMain.extract(request.mode);
        sendResponse({ content: extractedContent });
      } else {
        throw new Error("extract function is unavailable");
      }
    } catch (error) {
      console.error("WASM extraction error:", error);
      sendResponse({ content: "Extraction failed" });
    }

    return true; // keeps sendResponse available for async calls
  }
});
