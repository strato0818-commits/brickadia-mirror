import init, { validate_brz, process_brz } from "./pkg/brz_symmetry_web.js";

const fileInput = document.getElementById("fileInput");
const axisSelect = document.getElementById("axisSelect");
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
});

validateBtn.addEventListener("click", async () => {
  if (!currentFile) return;
  try {
    setStatus("Validating...");
    const bytes = await readFileBytes(currentFile);
    const result = validate_brz(bytes);
    setStatus(result);
  } catch (err) {
    setStatus(`Validation failed: ${err}`);
  }
});

processBtn.addEventListener("click", async () => {
  if (!currentFile) return;
  try {
    const axis = axisSelect.value;
    setStatus(`Processing axis ${axis.toUpperCase()}...`);
    const bytes = await readFileBytes(currentFile);
    const out = process_brz(bytes, axis);
    const stem = currentFile.name.replace(/\.brz$/i, "");
    const outName = `${stem}-${axis}.brz`;

    currentOutput = {
      name: outName,
      bytes: new Uint8Array(out),
    };

    downloadBytes(outName, out);
    setButtons();
    setStatus(`Done. Downloaded ${outName}. Use Copy Output to place it in clipboard.`);
  } catch (err) {
    setStatus(`Process failed: ${err}`);
  }
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
