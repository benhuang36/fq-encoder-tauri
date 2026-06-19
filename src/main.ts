import { invoke } from "@tauri-apps/api/core";
import { load, type Store } from "@tauri-apps/plugin-store";

const STORE_FILE = "settings.json";
const PASSWORD_KEY = "fq.encodingPassword";
const LANG_KEY = "fq.lang";

type Lang = "en" | "zh-Hant";

const STRINGS: Record<Lang, Record<string, string>> = {
  en: {
    subtitle: "Turn text into F U C K Y O u",
    settingsTitle: "Password settings",
    passwordLabel: "Encoding password",
    passwordPlaceholder: "Leave blank to use the default",
    revealTitle: "Show / hide password",
    passwordHint:
      "The same text encodes to a completely different string per password. Encoding and decoding must use the same password.",
    inputLabel: "Input",
    inputPlaceholder: "Type any text, or paste an encoded string…",
    outputLabel: "Output",
    outputPlaceholder: "The result appears here",
    copy: "Copy",
    copied: "Copied",
    footerHint: "Encode any text · Decode accepts only F U C K Y O u",
    invalid_length: "Length must be a multiple of 3 — this isn't a valid encoded string.",
    invalid_character: 'Contains an invalid character “{c}”. Encoded strings use only F U C K Y O u.',
    invalid_byte: "Invalid symbol combination — this wasn't produced by FQEncoder.",
    not_utf8: "Decode failed — wrong password, or not an FQEncoder string.",
  },
  "zh-Hant": {
    subtitle: "把文字變成 F U C K Y O u",
    settingsTitle: "設定密碼",
    passwordLabel: "編碼密碼",
    passwordPlaceholder: "留空使用預設密碼",
    revealTitle: "顯示／隱藏密碼",
    passwordHint:
      "相同文字在不同密碼下會編出完全不同的結果。編碼與解碼必須使用相同密碼。",
    inputLabel: "輸入",
    inputPlaceholder: "輸入任何文字，或貼上編碼字串…",
    outputLabel: "輸出",
    outputPlaceholder: "結果會顯示在這裡",
    copy: "複製",
    copied: "已複製",
    footerHint: "Encode 任意文字 · Decode 只接受 F U C K Y O u",
    invalid_length: "輸入長度必須是 3 的倍數，這不是合法的編碼字串。",
    invalid_character: "包含非法字元「{c}」，編碼字串只能由 F U C K Y O u 組成。",
    invalid_byte: "包含無效的編碼組合，這不是由 FQEncoder 產生的字串。",
    not_utf8: "解碼失敗，可能是密碼不正確，或這不是 FQEncoder 編出來的字串。",
  },
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const input = $<HTMLTextAreaElement>("input");
const output = $<HTMLTextAreaElement>("output");
const password = $<HTMLInputElement>("password");
const footer = $<HTMLElement>("footer");
const copyBtn = $<HTMLButtonElement>("copyBtn");

let store: Store | undefined;
let lang: Lang = "en";

function t(key: string): string {
  return STRINGS[lang][key] ?? key;
}

/** Localise a decode error code from Rust (e.g. "invalid_character:x"). */
function localiseError(code: string): string {
  const [kind, detail] = code.split(/:(.*)/s);
  const msg = t(kind);
  return detail ? msg.replace("{c}", detail) : msg;
}

function applyLang() {
  document.documentElement.lang = lang === "zh-Hant" ? "zh-Hant" : "en";
  // The toggle shows the language you'd switch TO.
  $("langBtn").textContent = lang === "en" ? "中" : "EN";

  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n!);
  });
  document.querySelectorAll<HTMLElement>("[data-i18n-placeholder]").forEach((el) => {
    (el as HTMLInputElement).placeholder = t(el.dataset.i18nPlaceholder!);
  });
  document.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle!);
  });

  copyBtn.textContent = t("copy");
  if (!footer.classList.contains("error")) footer.textContent = t("footerHint");
}

function detectLang(): Lang {
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-Hant" : "en";
}

function setOutput(text: string, isError = false) {
  output.value = isError ? "" : text;
  footer.textContent = isError ? text : t("footerHint");
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
  } catch (code) {
    setOutput(localiseError(String(code)), true);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  store = await load(STORE_FILE, { autoSave: true, defaults: {} });
  password.value = (await store.get<string>(PASSWORD_KEY)) ?? "";
  lang = (await store.get<Lang>(LANG_KEY)) ?? detectLang();
  applyLang();

  $("encodeBtn").addEventListener("click", runEncode);
  $("decodeBtn").addEventListener("click", runDecode);

  // ⌘/Ctrl+Enter = Encode, ⌘/Ctrl+Shift+Enter = Decode.
  input.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      e.shiftKey ? runDecode() : runEncode();
    }
  });

  $("langBtn").addEventListener("click", () => {
    lang = lang === "en" ? "zh-Hant" : "en";
    store?.set(LANG_KEY, lang);
    applyLang();
  });

  $("settingsBtn").addEventListener("click", () =>
    $("settings").classList.toggle("hidden"),
  );

  password.addEventListener("input", () => store?.set(PASSWORD_KEY, password.value));

  $("reveal").addEventListener("click", () => {
    password.type = password.type === "password" ? "text" : "password";
  });

  copyBtn.addEventListener("click", async () => {
    await navigator.clipboard.writeText(output.value);
    copyBtn.textContent = t("copied");
    setTimeout(() => (copyBtn.textContent = t("copy")), 1400);
  });
});
