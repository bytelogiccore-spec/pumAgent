<script lang="ts">
  import { appState, saveSettings, showError } from "../../lib/store.svelte";
  import { t, initLocales, localeManager } from "../../lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let isCustomLanguage = $state(false);
  let customLangInput = $state("");
  let isTranslating = $state(false);
  let extensions = $state<
    { name: string; description: string; enabled: boolean; source: string }[]
  >([]);
  let extensionLoading = $state(false);
  let extensionBusyMap = $state<Record<string, boolean>>({});

  async function loadExtensions() {
    extensionLoading = true;
    try {
      const items = await invoke<
        { name: string; description: string; enabled: boolean; source: string }[]
      >("list_extensions");
      extensions = items || [];
    } catch (e) {
      showError(`Failed to load extensions: ${e}`);
    } finally {
      extensionLoading = false;
    }
  }

  async function reloadExtensions() {
    extensionLoading = true;
    try {
      await invoke("reload_extensions");
      await loadExtensions();
    } catch (e) {
      showError(`Failed to reload extensions: ${e}`);
      extensionLoading = false;
    }
  }

  async function toggleExtension(name: string, enabled: boolean) {
    extensionBusyMap = { ...extensionBusyMap, [name]: true };
    try {
      await invoke("set_extension_enabled", { extensionName: name, enabled });
      extensions = extensions.map((ext) =>
        ext.name === name ? { ...ext, enabled } : ext,
      );
    } catch (e) {
      showError(`Failed to update extension '${name}': ${e}`);
      await loadExtensions();
    } finally {
      extensionBusyMap = { ...extensionBusyMap, [name]: false };
    }
  }

  onMount(() => {
    loadExtensions();
  });

  function onLanguageSelect(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    if (val === "custom") {
      isCustomLanguage = true;
    } else {
      isCustomLanguage = false;
      appState.config.language = val;
      saveSettings(false); // Do not close the modal
    }
  }

  async function generateCustomLang() {
    if (!appState.config.endpoints || appState.config.endpoints.length === 0) {
      showError(t("settings.api_not_set"));
      return;
    }

    let target = customLangInput.trim().toLowerCase();
    if (!target) return;

    isTranslating = true;
    try {
      await invoke("translate_i18n", { targetLang: target });

      if (!appState.config.customLanguages) {
        appState.config.customLanguages = [];
      }
      if (!appState.config.customLanguages.includes(target)) {
        appState.config.customLanguages.push(target);
      }

      appState.config.language = target;
      await initLocales();
      await saveSettings(false); // Do not close the modal
      isCustomLanguage = false; // Collapse custom form on success
      customLangInput = "";
    } catch (e) {
      console.error(e);
      showError(t("settings.trans_fail") + e);
    } finally {
      isTranslating = false;
    }
  }
</script>

{#if appState.sysModalDomain === "setting_server"}
<div class="form-group" style="padding-bottom: 8px; border-bottom: 1px solid #27272a; margin-bottom: 16px;">
  <label style="display:flex; justify-content:space-between; align-items:center;">
    <span>{t("settings.iface_lang")}</span>
    <select value={isCustomLanguage ? 'custom' : appState.config.language} onchange={onLanguageSelect} class="sys-select" style="width: 200px;">
      <option value="en">{t("settings.lang_en")}</option>
      <option value="ko">{t("settings.lang_ko")}</option>
      {#if appState.config.customLanguages}
        {#each appState.config.customLanguages as lang}
          <option value={lang}>{(localeManager.loadedLocales[lang] && localeManager.loadedLocales[lang]["settings.lang_custom_display"] && !localeManager.loadedLocales[lang]["settings.lang_custom_display"].includes("WRITE_ENGLISH_NAME") && localeManager.loadedLocales[lang]["settings.lang_custom_display"] !== "English(English)") ? localeManager.loadedLocales[lang]["settings.lang_custom_display"] : lang}</option>
        {/each}
      {/if}
      <option value="custom">{t("settings.lang_custom")}</option>
    </select>
  </label>
  {#if isCustomLanguage}
    <div style="margin-top: 12px; display:flex; gap: 8px; align-items:center;">
      <input type="text" bind:value={customLangInput} placeholder={t("settings.lang_ph")} style="flex:1;" />
      <button class="sys-badge" onclick={generateCustomLang} disabled={isTranslating} style="background: #1a1a1a; color: white;">
        {isTranslating ? t("settings.translating") : t("settings.trans_apply")}
      </button>
    </div>
  {/if}
</div>

<div class="form-group">
  <label>{t("form.maxLoops")}
    <input type="number" min="1" max="10" bind:value={appState.config.maxLoops} />
  </label>
</div>

<div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-top: 24px; margin-bottom: 8px;">{t("settings.kb_limits")}</div>
<div class="form-group" style="display:flex; flex-direction:row; gap: 16px;">
  <label style="flex: 1;">{t("settings.kb_rules")}
    <input type="number" min="500" max="10000" step="100" bind:value={appState.config.kbRulesTokenLimit} />
  </label>
  <label style="flex: 1;">{t("settings.kb_skills")}
    <input type="number" min="2000" max="50000" step="1000" bind:value={appState.config.kbSkillsTokenLimit} />
  </label>
</div>

<div class="form-group" style="margin-top: 16px;">
  <label>{t("form.searchProvider")}
    <select bind:value={appState.config.searchProvider} class="sys-select">
      <option value="duckduckgo">{t("settings.duckduckgo")}</option>
      <option value="tavily">Tavily Search API</option>
      <option value="google">Google Custom Search API</option>
    </select>
  </label>
</div>

<div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-top: 24px; margin-bottom: 8px;">Extensions</div>
<div class="form-group" style="border: 1px solid #1a1a1a; padding: 12px; background: #f8f6ef;">
  <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom: 10px;">
    <div style="font-size: 0.85rem; color:#1a1a1a; font-weight: 600;">External Tool Packages</div>
    <button class="sys-badge" onclick={reloadExtensions} disabled={extensionLoading}>
      {extensionLoading ? "Reloading..." : "Reload"}
    </button>
  </div>

  {#if extensionLoading && extensions.length === 0}
    <div style="font-size: 0.85rem; color:#52525b;">Loading extensions...</div>
  {:else if extensions.length === 0}
    <div style="font-size: 0.85rem; color:#52525b;">No extensions found.</div>
  {:else}
    <div style="display:flex; flex-direction:column; gap:10px;">
      {#each extensions as ext}
        <div style="border:1px solid #d4d4d8; padding:10px; background:#ffffff;">
          <div style="display:flex; justify-content:space-between; align-items:flex-start; gap:12px;">
            <div style="min-width:0;">
              <div style="font-size:0.9rem; color:#111827; font-weight:700; word-break:break-word;">
                {ext.name}
              </div>
              <div style="font-size:0.8rem; color:#4b5563; margin-top:2px; word-break:break-word;">
                {ext.description || "External extension tool"}
              </div>
            </div>
            <label style="display:flex; align-items:center; gap:6px; margin:0; color:#111827; cursor:pointer; text-transform:none; letter-spacing:0;">
              <input
                type="checkbox"
                checked={ext.enabled}
                disabled={!!extensionBusyMap[ext.name]}
                onchange={(e) => toggleExtension(ext.name, (e.target as HTMLInputElement).checked)}
                style="width:auto;"
              />
              <span style="font-size:0.8rem;">{ext.enabled ? "Enabled" : "Disabled"}</span>
            </label>
          </div>
          <div style="font-size:0.72rem; color:#6b7280; margin-top:6px; word-break:break-all;">
            {ext.source}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
{#if appState.config.searchProvider === "tavily"}
  <div class="form-group">
    <label>{t("form.tavilyKey")}
      <input type="password" placeholder="tvly-..." bind:value={appState.config.tavilyApiKey} />
    </label>
  </div>
{/if}
{#if appState.config.searchProvider === "google"}
  <div class="form-group">
    <label>{t("form.googleKey")}
      <input type="password" bind:value={appState.config.googleApiKey} />
    </label>
  </div>
  <div class="form-group">
    <label>{t("form.googleCx")}
      <input type="text" bind:value={appState.config.googleCx} />
    </label>
  </div>
{/if}
{/if}

{#if appState.sysModalDomain === "setting_heartbeat"}
<div class="form-group" style="display:flex; flex-direction:row; align-items:center; gap:8px;">
  <input type="checkbox" id="heartbeat-toggle" bind:checked={appState.config.heartbeatEnabled} style="width: auto;" />
  <label for="heartbeat-toggle" style="margin: 0; cursor: pointer;">{t("form.heartbeatToggle")}</label>
</div>
<div class="form-group" style="margin-top: 12px;">
  <label>{t("form.heartbeatInterval")}
    <input type="number" min="10" max="86400" bind:value={appState.config.heartbeatInterval} placeholder="e.g. 3600" />
  </label>
</div>
<div class="form-group">
  <label>{t("form.heartbeatPrompt")}
    <textarea bind:value={appState.config.heartbeatPrompt} rows="3"></textarea>
  </label>
  <div style="font-size:0.8rem; color:#a1a1aa; margin-top:4px;">{t("form.heartbeatDesc")}</div>
</div>
{/if}

{#if appState.sysModalDomain === "setting_telegram"}
<div class="form-group" style="display:flex; flex-direction:row; align-items:center; gap:8px;">
  <input type="checkbox" id="telegram-toggle" bind:checked={appState.config.telegramEnabled} style="width: auto;" />
  <label for="telegram-toggle" style="margin: 0; cursor: pointer;">{t("form.telegramToggle")}</label>
</div>
<div class="form-group" style="margin-top: 12px;">
  <label>{t("form.telegramToken")}
    <input type="password" bind:value={appState.config.telegramBotToken} placeholder="123456789:AAHx..." />
  </label>
  <div style="font-size:0.8rem; color:#ef4444; margin-top:4px; font-weight:600;">{t("form.telegramDesc")}</div>
  {#if appState.config.telegramChatId}
    <div style="font-size:0.85rem; color:#10b981; margin-top:8px; font-weight:600;">{t("settings.telegram_ok", { id: appState.config.telegramChatId })}</div>
  {:else}
    <div style="font-size:0.85rem; color:#f59e0b; margin-top:8px; font-weight:600;">{t("settings.telegram_wait")}</div>
  {/if}
</div>
{/if}

{#if isTranslating}
  <div class="blur-overlay">
    <div style="display: flex; flex-direction: column; align-items: center; gap: 24px;">
      <svg class="brain-loader mega" viewBox="0 0 100 100" width="160" height="160" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <!-- Hexagon -->
        <path class="brain-line" d="M 50 5 L 85 25 L 85 69 L 50 89 L 15 69 L 15 25 Z" />
        <circle class="brain-line" cx="50" cy="5" r="3" />
        <circle class="brain-line" cx="85" cy="25" r="3" />
        <circle class="brain-line" cx="85" cy="69" r="3" />
        <circle class="brain-line" cx="50" cy="89" r="3" />
        <circle class="brain-line" cx="15" cy="69" r="3" />
        <circle class="brain-line" cx="15" cy="25" r="3" />
        <!-- Connectors -->
        <path class="brain-line" d="M 50 8 L 50 22" />
        <path class="brain-line" d="M 50 86 L 50 72" />
        <path class="brain-line" d="M 82 27 L 67 34" />
        <path class="brain-line" d="M 82 67 L 66 65" />
        <path class="brain-line" d="M 18 27 L 33 34" />
        <path class="brain-line" d="M 18 67 L 34 65" />
        <!-- Fissure -->
        <path class="brain-line" d="M 50 22 L 50 72" />
        <!-- Right Hemisphere -->
        <path class="brain-line" d="M 50 22 C 58 18, 68 25, 67 34 C 76 34, 78 45, 69 49 C 76 53, 76 65, 66 65 C 67 72, 58 75, 50 72" />
        <path class="brain-line" d="M 54 32 C 60 33, 64 38, 62 43" />
        <path class="brain-line" d="M 54 45 C 61 42, 66 50, 64 60" />
        <path class="brain-line" d="M 52 57 C 57 56, 60 60, 58 68" />
        <!-- Left Hemisphere -->
        <path class="brain-line" d="M 50 22 C 42 18, 32 25, 33 34 C 24 34, 22 45, 31 49 C 24 53, 24 65, 34 65 C 33 72, 42 75, 50 72" />
        <path class="brain-line" d="M 46 32 C 40 33, 36 38, 38 43" />
        <path class="brain-line" d="M 46 45 C 39 42, 34 50, 36 60" />
        <path class="brain-line" d="M 48 57 C 43 56, 40 60, 42 68" />
      </svg>
      <div style="font-size: 1.25rem; font-weight: 700; color: #7df9ff; letter-spacing: 0.1em; text-transform: uppercase;">
        {t("settings.translating")}
      </div>
    </div>
  </div>
{/if}

<style>
  .blur-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(16px);
    z-index: 999999;
    display: flex;
    justify-content: center;
    align-items: center;
    color: #7df9ff;
  }
  .brain-loader.mega {
    filter: drop-shadow(0 0 6px rgba(0, 255, 255, 0.6)) drop-shadow(0 0 16px rgba(0, 255, 255, 0.3));
  }
  .form-group label {
    color: #a1a1aa;
    font-weight: 600;
    font-size: 0.85rem;
    margin-bottom: 8px;
    display: block;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .form-group input,
  .form-group textarea,
  .sys-select {
    width: 100%;
    box-sizing: border-box;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 12px;
    border-radius: 0;
    color: #1a1a1a;
    font-family: inherit;
    font-size: 1rem;
    resize: none;
  }
  .form-group input:focus,
  .form-group textarea:focus,
  .sys-select:focus {
    outline: none;
    border-color: #e0005a;
  }
  .sys-badge {
    flex-shrink: 0;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 6px 16px;
    border-radius: 0;
    color: #1a1a1a;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: 0.2s;
  }
  .sys-badge:hover:not(:disabled) {
    background: #e0005a;
    color: #fcfbf8;
    border-color: #e0005a;
  }
  .sys-badge:disabled {
    opacity: 0.8;
    cursor: wait;
  }
  .brain-loader {
    animation: pulseBrain 1.5s infinite ease-in-out;
    filter: drop-shadow(0 0 2px rgba(255, 255, 255, 0.5));
  }
  .brain-line {
    stroke-dasharray: 200;
    stroke-dashoffset: 200;
    animation: drawBrain 1.5s linear infinite alternate;
  }
  @keyframes drawBrain {
    0% { stroke-dashoffset: 200; }
    100% { stroke-dashoffset: 0; }
  }
  @keyframes pulseBrain {
    0%, 100% { transform: scale(1); opacity: 0.85; }
    50% { transform: scale(1.05); opacity: 1; }
  }
</style>
