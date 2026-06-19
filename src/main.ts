import { invoke } from "@tauri-apps/api/core";
import { load, type Store } from "@tauri-apps/plugin-store";

const STORE_FILE = "settings.json";
const PASSWORD_KEY = "fq.encodingPassword";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const input = $<HTMLTextAreaElement>("input");
const output = $<HTMLTextAreaElement>("output");
const password = $<HTMLInputElement>("password");
const footer = $<HTMLElement>("footer");
const copyBtn = $<HTMLButtonElement>("copyBtn");

let store: Store | undefined;

async function initStore() {
  store = await load(STORE_FILE, { autoSave: true, defaults: {} });
  password.value = (await store.get<string>(PASSWORD_KEY)) ?? "";
}

function setOutput(text: string, isError = false) {
  output.value = isError ? "" : text;
  footer.textContent = isError
    ? text
    : "Encode 任意文字 · Decode 只接受 F U C K Y O u";
  footer.classList.toggle("error", isError);
  copyBtn.classList.toggle("hidden", isError || text.length === 0);
}

async function runEncode() {
  setOutput(await invoke<string>("encode", { text: input.value, key: password.value }));
}

async function runDecode() {
  try {
    const result = await invoke<string>("decode", {
      text: input.value.trim(),
      key: password.value,
    });
    setOutput(result);
  } catch (e) {
    setOutput(String(e), true);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  initStore();

  $("encodeBtn").addEventListener("click", runEncode);
  $("decodeBtn").addEventListener("click", runDecode);

  // ⌘/Ctrl+Enter = Encode, ⌘/Ctrl+Shift+Enter = Decode.
  input.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      e.shiftKey ? runDecode() : runEncode();
    }
  });

  $("settingsBtn").addEventListener("click", () =>
    $("settings").classList.toggle("hidden"),
  );

  password.addEventListener("input", () => {
    store?.set(PASSWORD_KEY, password.value);
  });

  $("reveal").addEventListener("click", () => {
    password.type = password.type === "password" ? "text" : "password";
  });

  copyBtn.addEventListener("click", async () => {
    await navigator.clipboard.writeText(output.value);
    copyBtn.textContent = "已複製";
    setTimeout(() => (copyBtn.textContent = "複製"), 1400);
  });
});
