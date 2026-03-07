import init, { validate_brz, process_brz } from "./pkg/brz_symmetry_web.js";

const fileInput = document.getElementById("fileInput");
const axisSelect = document.getElementById("axisSelect");
const offsetInput = document.getElementById("offsetInput");
const autoActionToggle = document.getElementById("autoActionToggle");
const validateBtn = document.getElementById("validateBtn");
const processBtn = document.getElementById("processBtn");
const copyOutputBtn = document.getElementById("copyOutputBtn");
const statusEl = document.getElementById("status");

let wasmReady = false;
let currentFile = null;
let currentOutput = null;

function setStatus(message) {
  statusEl.textContent = message;
}

function setButtons() {
  const fileReady = wasmReady && currentFile !== null;
  validateBtn.disabled = !fileReady;
  processBtn.disabled = !fileReady;
  copyOutputBtn.disabled = !wasmReady || currentOutput === null;
}

function clearOutput() {
  currentOutput = null;
  setButtons();
}

function setInputFile(file, source) {
  currentFile = file;
  clearOutput();
  setButtons();
  setStatus(`${source}: ${file.name}`);
}

async function tryParseCurrentFile(contextLabel) {
  if (!wasmReady || !currentFile) return;

  try {
    setStatus(`${contextLabel}: parsing...`);
    const bytes = await readFileBytes(currentFile);
    const result = validate_brz(bytes);
    setStatus(`${contextLabel}: ${result}`);
  } catch (err) {
    setStatus(`${contextLabel}: parse failed: ${err}`);
  }
}

async function readFileBytes(file) {
  const buf = await file.arrayBuffer();
  return new Uint8Array(buf);
}

function downloadBytes(name, bytes) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

function toBrzName(nameHint) {
  if (!nameHint) return `clipboard-${Date.now()}.brz`;
  return nameHint.toLowerCase().endsWith(".brz") ? nameHint : `${nameHint}.brz`;
}

function fileLooksLikeBrz(file) {
  const type = (file.type || "").toLowerCase();
  const name = (file.name || "").toLowerCase();
  return (
    name.endsWith(".brz") ||
    type.includes("brz") ||
    type === "application/octet-stream" ||
    type === "application/x-brz"
  );
}

function extractClipboardFile(clipboardData) {
  const files = Array.from(clipboardData.files || []);
  const preferred = files.find(fileLooksLikeBrz);
  if (preferred) return preferred;
  if (files.length === 1) return files[0];

  for (const item of Array.from(clipboardData.items || [])) {
    if (item.kind !== "file") continue;
    const file = item.getAsFile();
    if (file && fileLooksLikeBrz(file)) return file;
  }

  return null;
}

async function copyOutputToClipboard() {
  if (!currentOutput) return;
  if (typeof ClipboardItem === "undefined") {
    setStatus("Clipboard write is not supported in this browser.");
    return;
  }

  try {
    const blob = new Blob([currentOutput.bytes], { type: "application/octet-stream" });
    const supports = typeof ClipboardItem.supports === "function" ? ClipboardItem.supports.bind(ClipboardItem) : null;
    const supportedBinaryType = ["application/x-brz", "application/octet-stream", "web application/octet-stream"]
      .find((type) => (supports ? supports(type) : false));

    if (supportedBinaryType && navigator.clipboard?.write) {
      const item = new ClipboardItem({ [supportedBinaryType]: blob });
      await navigator.clipboard.write([item]);
      setStatus(`Copied ${currentOutput.name} to clipboard.`);
      return;
    }

    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(currentOutput.name);
      setStatus(
        "This browser cannot write BRZ binary data to clipboard. Output filename copied instead."
      );
      return;
    }

    setStatus("Clipboard write is not supported in this browser.");
  } catch (err) {
    setStatus(`Copy failed: ${err}`);
  }
}

async function processCurrentFile(contextLabel, isAutoAction = false) {
  if (!currentFile) return;

  try {
    const axis = axisSelect.value;
    const zOffsetRaw = Number.parseInt(offsetInput?.value ?? "6", 10);
    const zOffset = Number.isFinite(zOffsetRaw) ? zOffsetRaw : 6;
    setStatus(`${contextLabel}: processing axis ${axis.toUpperCase()} (z offset ${zOffset})...`);
    const bytes = await readFileBytes(currentFile);
    const out = process_brz(bytes, axis, zOffset);
    const stem = currentFile.name.replace(/\.brz$/i, "");
    const outName = `${stem}-${axis}.brz`;

    currentOutput = {
      name: outName,
      bytes: new Uint8Array(out),
    };

    downloadBytes(outName, out);
    setButtons();
    if (isAutoAction) {
      setStatus(`Done. Auto-downloaded ${outName}.`);
    } else {
      setStatus(`Done. Downloaded ${outName}. Use Copy Output to place it in clipboard.`);
    }
  } catch (err) {
    setStatus(`Process failed: ${err}`);
  }
}

fileInput.addEventListener("change", () => {
  const selected = fileInput.files?.[0] ?? null;
  if (!selected) {
    currentFile = null;
    clearOutput();
    setButtons();
    setStatus("Paste with Ctrl+V, or select a BRZ file.");
    return;
  }

  setInputFile(selected, "Selected");
});

window.addEventListener("paste", (event) => {
  const clipboardData = event.clipboardData;
  if (!clipboardData) return;

  const file = extractClipboardFile(clipboardData);
  if (!file) return;

  event.preventDefault();
  const brzFile = new File([file], toBrzName(file.name || "clipboard"), {
    type: file.type || "application/octet-stream",
  });

  fileInput.value = "";
  setInputFile(brzFile, "Pasted");
  if (autoActionToggle?.checked) {
    void processCurrentFile("Auto action", true);
  } else {
    void tryParseCurrentFile("Pasted file");
  }
});

validateBtn.addEventListener("click", async () => {
  if (!currentFile) return;
  await tryParseCurrentFile("Validation");
});

processBtn.addEventListener("click", async () => {
  await processCurrentFile("Process");
});

copyOutputBtn.addEventListener("click", copyOutputToClipboard);

(async () => {
  try {
    await init();
    wasmReady = true;
    setButtons();
    setStatus("WASM ready. Paste with Ctrl+V, or select a BRZ file.");
  } catch (err) {
    setStatus(`Failed to initialize wasm: ${err}`);
  }
})();
