import { appState, addLog } from "./store.svelte";
import { invoke } from "@tauri-apps/api/core";

import defaultEn from "../locales/en.json";
import defaultKo from "../locales/ko.json";

export const defaultLocales: Record<string, Record<string, string>> = {
  en: defaultEn,
  ko: defaultKo,
};

export const localeManager = $state({
  loadedLocales: {} as Record<string, Record<string, string>>,
});

export async function initLocales() {
  try {
    let files: string[] = await invoke("list_knowledge", { domain: "locales" });

    // Provide default files if missing
    if (!files.includes("en.json")) {
      await invoke("write_knowledge", { domain: "locales", name: "en.json", content: JSON.stringify(defaultLocales["en"], null, 2) });
    }
    if (!files.includes("ko.json")) {
      await invoke("write_knowledge", { domain: "locales", name: "ko.json", content: JSON.stringify(defaultLocales["ko"], null, 2) });
    }

    files = await invoke("list_knowledge", { domain: "locales" });

    for (let file of files) {
      if (!file.endsWith(".json")) continue;
      let lang = file.replace(".json", "");
      let content: string = await invoke("read_knowledge", { domain: "locales", name: file });
      if (content) {
        try {
          localeManager.loadedLocales[lang] = JSON.parse(content);
        } catch (e) {
          console.error(`Invalid JSON in locale ${file}`);
        }
      }
    }

    addLog(`[i18n] Successfully initialized locales: ${Object.keys(localeManager.loadedLocales).join(", ")}`);

  } catch (err) {
    console.error("Failed to init locales:", err);
    // Fallback just in case rust backend fails
    localeManager.loadedLocales = { ...defaultLocales };
  }
}

export function t(key: string, variables?: Record<string, string | number>): string {
  let lang = appState.config?.language || "en";
  let dictionary = localeManager.loadedLocales[lang] || localeManager.loadedLocales["en"] || defaultLocales["en"];
  let text = dictionary[key] || (defaultLocales[lang] && defaultLocales[lang][key]) || defaultLocales["en"][key] || key;

  if (variables) {
    for (const [k, v] of Object.entries(variables)) {
      text = text.replace(`{${k}}`, String(v));
    }
  }
  return text;
}
