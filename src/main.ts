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
    tabCodec: "Codec",
    tabAvalanche: "Avalanche",
    tabStego: "Stego",
    inputLabel: "Input",
    inputPlaceholder: "Type any text, or paste an encoded string…",
    outputLabel: "Output",
    outputPlaceholder: "The result appears here",
    copy: "Copy",
    copied: "Copied",
    qrBtn: "QR",
    qr_too_long: "Text is too long to fit in a QR code.",
    footerHint: "Encode any text · Decode accepts only F U C K Y O u",
    invalid_length: "Length must be a multiple of 3 — this isn't a valid encoded string.",
    invalid_character: 'Contains an invalid character “{c}”. Encoded strings use only F U C K Y O u.',
    invalid_byte: "Invalid symbol combination — this wasn't produced by FQEncoder.",
    not_utf8: "Decode failed — wrong password, or not an FQEncoder string.",
    // Avalanche
    avInputLabel: "Text",
    avInputPlaceholder: "Type, then flip a character and watch the output diffuse…",
    avFlip: "🎲 Flip a random character",
    avOutputLabel: "Encoded output",
    avHint: "Changing one character avalanches through most of the output — that's the bidirectional diffusion stage at work.",
    avEmpty: "Type something to begin.",
    avChanged: "symbols changed",
    // Stego
    stSecretLabel: "Secret message",
    stSecretPlaceholder: "What to hide…",
    stCoverLabel: "Cover text",
    stCoverPlaceholder: "An innocent-looking sentence",
    stEncodeFirst: "First encode to FUCKYOu (double layer)",
    stHideBtn: "🫥 Hide",
    stHiddenLabel: "Hidden output — looks like the cover, copy it!",
    stRevealLabel: "Hidden text to inspect",
    stRevealPlaceholder: "Paste text that may carry a hidden message…",
    stRevealBtn: "🔍 Reveal",
    stRevealOutLabel: "Revealed message",
    stego_no_payload: "No hidden message found in this text.",
    stego_corrupt: "The hidden data looks corrupted.",
    stego_not_utf8: "Couldn't decode the hidden message.",
  },
  "zh-Hant": {
    subtitle: "把文字變成 F U C K Y O u",
    settingsTitle: "設定密碼",
    passwordLabel: "編碼密碼",
    passwordPlaceholder: "留空使用預設密碼",
    revealTitle: "顯示／隱藏密碼",
    passwordHint: "相同文字在不同密碼下會編出完全不同的結果。編碼與解碼必須使用相同密碼。",
    tabCodec: "編解碼",
    tabAvalanche: "雪崩",
    tabStego: "隱寫",
    inputLabel: "輸入",
    inputPlaceholder: "輸入任何文字，或貼上編碼字串…",
    outputLabel: "輸出",
    outputPlaceholder: "結果會顯示在這裡",
    copy: "複製",
    copied: "已複製",
    qrBtn: "QR",
    qr_too_long: "文字太長，無法放進 QR code。",
    footerHint: "Encode 任意文字 · Decode 只接受 F U C K Y O u",
    invalid_length: "輸入長度必須是 3 的倍數，這不是合法的編碼字串。",
    invalid_character: "包含非法字元「{c}」，編碼字串只能由 F U C K Y O u 組成。",
    invalid_byte: "包含無效的編碼組合，這不是由 FQEncoder 產生的字串。",
    not_utf8: "解碼失敗，可能是密碼不正確，或這不是 FQEncoder 編出來的字串。",
    // Avalanche
    avInputLabel: "文字",
    avInputPlaceholder: "打點字，再改一個字元，看輸出雪崩…",
    avFlip: "🎲 隨機改一個字元",
    avOutputLabel: "編碼輸出",
    avHint: "改動一個字元會讓大部分輸出跟著翻動 —— 這就是雙向擴散階段的效果。",
    avEmpty: "輸入一些文字開始。",
    avChanged: "個符號改變",
    // Stego
    stSecretLabel: "祕密訊息",
    stSecretPlaceholder: "想藏起來的內容…",
    stCoverLabel: "掩護文字",
    stCoverPlaceholder: "一句看起來很正常的話",
    stEncodeFirst: "先用 FUCKYOu 編碼（雙層）",
    stHideBtn: "🫥 藏起來",
    stHiddenLabel: "隱藏結果 —— 看起來就是掩護文字，複製它！",
    stRevealLabel: "要檢查的文字",
    stRevealPlaceholder: "貼上可能藏有訊息的文字…",
    stRevealBtn: "🔍 還原",
    stRevealOutLabel: "還原出的訊息",
    stego_no_payload: "這段文字裡找不到隱藏訊息。",
    stego_corrupt: "隱藏資料似乎損壞了。",
    stego_not_utf8: "無法解出隱藏訊息。",
  },
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const password = $<HTMLInputElement>("password");
const footer = $<HTMLElement>("footer");

let store: Store | undefined;
let lang: Lang = "en";

const t = (key: string) => STRINGS[lang][key] ?? key;

/** Localise an error code from Rust (e.g. "invalid_character:x"). */
function localiseError(code: string): string {
  const [kind, detail] = code.split(/:(.*)/s);
  const msg = t(kind);
  return detail ? msg.replace("{c}", detail) : msg;
}

function applyLang() {
  document.documentElement.lang = lang === "zh-Hant" ? "zh-Hant" : "en";
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
  $("copyBtn").textContent = t("copy");
  $("stHiddenCopy").textContent = t("copy");
  if (!footer.classList.contains("error")) footer.textContent = t("footerHint");
  if (currentTab === "avalanche") renderAvalanche();
}

const detectLang = (): Lang =>
  navigator.language.toLowerCase().startsWith("zh") ? "zh-Hant" : "en";

async function copyToClipboard(text: string, btn: HTMLButtonElement) {
  await navigator.clipboard.writeText(text);
  btn.textContent = t("copied");
  setTimeout(() => (btn.textContent = t("copy")), 1400);
}

// ── Tabs ──────────────────────────────────────────────────────────────
type Tab = "codec" | "avalanche" | "stego";
let currentTab: Tab = "codec";

function setTab(tab: Tab) {
  currentTab = tab;
  document.querySelectorAll<HTMLElement>(".tab").forEach((b) =>
    b.classList.toggle("active", b.dataset.tab === tab),
  );
  document.querySelectorAll<HTMLElement>(".tab-panel").forEach((p) =>
    p.classList.toggle("hidden", p.id !== `tab-${tab}`),
  );
  if (tab === "avalanche") renderAvalanche();
}

// ── Codec ─────────────────────────────────────────────────────────────
const input = $<HTMLTextAreaElement>("input");
const output = $<HTMLTextAreaElement>("output");
const copyBtn = $<HTMLButtonElement>("copyBtn");
const qrBtn = $<HTMLButtonElement>("qrBtn");
const qrBox = $<HTMLElement>("qrBox");

function setOutput(text: string, isError = false) {
  output.value = isError ? "" : text;
  footer.textContent = isError ? text : t("footerHint");
  footer.classList.toggle("error", isError);
  const empty = isError || text.length === 0;
  copyBtn.classList.toggle("hidden", empty);
  qrBtn.classList.toggle("hidden", empty);
  qrBox.classList.add("hidden"); // a new result invalidates the old QR
  qrBox.replaceChildren();
}

async function toggleQr() {
  if (!qrBox.classList.contains("hidden")) {
    qrBox.classList.add("hidden");
    return;
  }
  try {
    qrBox.innerHTML = await invoke<string>("qr_svg", { text: output.value });
    qrBox.classList.remove("hidden");
  } catch (code) {
    setOutput(localiseError(String(code)), true);
  }
}

async function runEncode() {
  setOutput(await invoke<string>("encode", { text: input.value, key: password.value }));
}

async function runDecode() {
  try {
    setOutput(await invoke<string>("decode", { text: input.value.trim(), key: password.value }));
  } catch (code) {
    setOutput(localiseError(String(code)), true);
  }
}

// ── Avalanche ─────────────────────────────────────────────────────────
const avInput = $<HTMLTextAreaElement>("avInput");
const avGrid = $<HTMLElement>("avGrid");
const avStat = $<HTMLElement>("avStat");
let avPrev: string[] = [];

async function renderAvalanche() {
  const text = avInput.value;
  if (text.length === 0) {
    avGrid.textContent = t("avEmpty");
    avGrid.classList.add("empty");
    avStat.textContent = "";
    avPrev = [];
    return;
  }
  avGrid.classList.remove("empty");
  const out = await invoke<string>("encode", { text, key: password.value });
  const symbols = [...out];

  avGrid.replaceChildren();
  let changed = 0;
  symbols.forEach((sym, i) => {
    const cell = document.createElement("span");
    cell.className = "cell";
    cell.textContent = sym;
    if (avPrev.length && avPrev[i] !== sym) {
      cell.classList.add("flash");
      changed++;
    } else if (i >= avPrev.length && avPrev.length) {
      changed++;
    }
    avGrid.appendChild(cell);
  });

  const pct = symbols.length ? Math.round((changed / symbols.length) * 100) : 0;
  avStat.textContent = avPrev.length ? `${changed} / ${symbols.length} ${t("avChanged")} · ${pct}%` : "";
  avPrev = symbols;
}

function flipRandomChar() {
  const chars = [...avInput.value];
  if (chars.length === 0) return;
  const i = Math.floor(Math.random() * chars.length);
  const base = chars[i].charCodeAt(0);
  let replacement = chars[i];
  while (replacement === chars[i]) {
    replacement = String.fromCharCode(33 + ((base + Math.floor(Math.random() * 90) - 33 + 1) % 90));
  }
  chars[i] = replacement;
  avInput.value = chars.join("");
  renderAvalanche();
}

// ── Stego ─────────────────────────────────────────────────────────────
const stHidden = $<HTMLTextAreaElement>("stHidden");
const stHiddenCopy = $<HTMLButtonElement>("stHiddenCopy");
const stRevealOut = $<HTMLTextAreaElement>("stRevealOut");
const stStatus = $<HTMLElement>("stStatus");

async function runHide() {
  let secret = $<HTMLTextAreaElement>("stSecret").value;
  if (secret.length === 0) return;
  if ($<HTMLInputElement>("stEncodeFirst").checked) {
    secret = await invoke<string>("encode", { text: secret, key: password.value });
  }
  const cover = $<HTMLTextAreaElement>("stCover").value;
  const hidden = await invoke<string>("stego_hide", { secret, cover });
  stHidden.value = hidden;
  stHiddenCopy.classList.remove("hidden");
}

async function runReveal() {
  stStatus.textContent = "";
  stStatus.classList.remove("error");
  try {
    stRevealOut.value = await invoke<string>("stego_reveal", {
      text: $<HTMLTextAreaElement>("stRevealIn").value,
    });
  } catch (code) {
    stRevealOut.value = "";
    stStatus.textContent = localiseError(String(code));
    stStatus.classList.add("error");
  }
}

// ── Wire up ───────────────────────────────────────────────────────────
window.addEventListener("DOMContentLoaded", async () => {
  store = await load(STORE_FILE, { autoSave: true, defaults: {} });
  password.value = (await store.get<string>(PASSWORD_KEY)) ?? "";
  lang = (await store.get<Lang>(LANG_KEY)) ?? detectLang();
  applyLang();

  document.querySelectorAll<HTMLElement>(".tab").forEach((b) =>
    b.addEventListener("click", () => setTab(b.dataset.tab as Tab)),
  );

  $("encodeBtn").addEventListener("click", runEncode);
  $("decodeBtn").addEventListener("click", runDecode);
  input.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      e.shiftKey ? runDecode() : runEncode();
    }
  });

  avInput.addEventListener("input", renderAvalanche);
  $("avFlipBtn").addEventListener("click", flipRandomChar);

  $("stHideBtn").addEventListener("click", runHide);
  $("stRevealBtn").addEventListener("click", runReveal);
  stHiddenCopy.addEventListener("click", () => copyToClipboard(stHidden.value, stHiddenCopy));

  $("langBtn").addEventListener("click", () => {
    lang = lang === "en" ? "zh-Hant" : "en";
    store?.set(LANG_KEY, lang);
    applyLang();
  });
  $("settingsBtn").addEventListener("click", () => $("settings").classList.toggle("hidden"));
  password.addEventListener("input", () => store?.set(PASSWORD_KEY, password.value));
  $("reveal").addEventListener("click", () => {
    password.type = password.type === "password" ? "text" : "password";
  });
  copyBtn.addEventListener("click", () => copyToClipboard(output.value, copyBtn));
  qrBtn.addEventListener("click", toggleQr);
});
