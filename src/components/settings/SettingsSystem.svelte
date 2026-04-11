<script lang="ts">
  import { appState, saveSettings, showError } from "../../lib/store.svelte";
  import { t, initLocales } from "../../lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";

  let isCustomLanguage = $state(false);
  let customLangInput = $state("");
  let isTranslating = $state(false);

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
    if (!appState.config.endpoints || appState.config.endpoints.length === 0 || !appState.config.endpoints[0].api_key) {
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
          <option value={lang}>{lang.charAt(0).toUpperCase() + lang.slice(1)}{t("settings.lang_ai")}</option>
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

<style>
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
  .sys-badge:hover {
    background: #e0005a;
    color: #fcfbf8;
    border-color: #e0005a;
  }
</style>
