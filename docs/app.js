import init, { validate_brz, process_brz_roundtrip } from "./pkg/brz_symmetry_web.js";

const fileInput = document.getElementById("fileInput");
const validateBtn = document.getElementById("validateBtn");
const processBtn = document.getElementById("processBtn");
const statusEl = document.getElementById("status");

let wasmReady = false;
let currentFile = null;

function setStatus(message) {
  statusEl.textContent = message;
}

function setButtons() {
  const enabled = wasmReady && currentFile !== null;
  validateBtn.disabled = !enabled;
  processBtn.disabled = !enabled;
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

fileInput.addEventListener("change", () => {
  currentFile = fileInput.files?.[0] ?? null;
  setButtons();
  setStatus(currentFile ? `Selected: ${currentFile.name}` : "Select a BRZ file.");
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
    setStatus("Processing...");
    const bytes = await readFileBytes(currentFile);
    const out = process_brz_roundtrip(bytes);
    const stem = currentFile.name.replace(/\.brz$/i, "");
    downloadBytes(`${stem}-processed.brz`, out);
    setStatus(`Done. Downloaded ${stem}-processed.brz`);
  } catch (err) {
    setStatus(`Process failed: ${err}`);
  }
});

(async () => {
  try {
    await init();
    wasmReady = true;
    setButtons();
    setStatus("WASM ready. Select a BRZ file.");
  } catch (err) {
    setStatus(`Failed to initialize wasm: ${err}`);
  }
})();
