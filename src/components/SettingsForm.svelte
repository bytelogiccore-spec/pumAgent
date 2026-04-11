<script lang="ts">
  import { DEFAULT_PLANNER, DEFAULT_CRITIC, DEFAULT_WRITER, DEFAULT_REFLECTOR, DEFAULT_HEARTBEAT, DEFAULT_WORKER, DEFAULT_REGISTRY } from "../lib/constants";
  import { appState, saveSettings } from "../lib/store.svelte";
  import { t, initLocales } from "../lib/i18n.svelte";
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
      saveSettings();
    }
  }

  async function generateCustomLang() {
    if (!customLangInput.trim()) return;
    isTranslating = true;
    try {
      await invoke("translate_i18n", { targetLang: customLangInput.trim().toLowerCase() });
      appState.config.language = customLangInput.trim().toLowerCase();
      await initLocales();
      await saveSettings();
      isCustomLanguage = false; // Collapse custom form on success
      customLangInput = "";
    } catch (e) {
      console.error(e);
      alert("Translation failed: " + e);
    } finally {
      isTranslating = false;
    }
  }
</script>

<div class="settings-body" style="width: 100%; height: 100%; overflow-y: auto; position: relative;">
  
  {#if appState.sysModalDomain === "setting_server"}
  <div class="form-group" style="padding-bottom: 8px; border-bottom: 1px solid #27272a; margin-bottom: 16px;">
    <label style="display:flex; justify-content:space-between; align-items:center;">
      <span>🌍 Interface Language</span>
      <select value={isCustomLanguage ? 'custom' : appState.config.language} onchange={onLanguageSelect} class="sys-select" style="width: 200px;">
        <option value="en">English (Default)</option>
        <option value="ko">한국어 (Korean)</option>
        <option value="es">Español (スペイン語)</option>
        <option value="fr">Français (French)</option>
        <option value="de">Deutsch (German)</option>
        <option value="zh">中文 (Chinese)</option>
        <option value="ja">日本語 (Japanese)</option>
        <option value="custom">✨ Generate via AI...</option>
      </select>
    </label>
    {#if isCustomLanguage}
      <div style="margin-top: 12px; display:flex; gap: 8px; align-items:center;">
        <input type="text" bind:value={customLangInput} placeholder="Enter any language (e.g., Italian, ru, etc.)" style="flex:1;" />
        <button class="sys-badge" onclick={generateCustomLang} disabled={isTranslating} style="background: #1a1a1a; color: white;">
          {isTranslating ? "Translating..." : "Translate & Apply"}
        </button>
      </div>
    {/if}
  </div>
  <div class="form-group">
    <label>{t("form.apiUrl")}
      <input type="text" bind:value={appState.config.apiUrl} />
    </label>
  </div>
  <div class="form-group">
    <label>{t("form.model")}
      <input type="text" bind:value={appState.config.model} />
    </label>
  </div>
  
  <div style="margin-bottom: 24px; padding: 12px; border: 1px dashed #3f3f46; border-radius: 4px; background: rgba(0,0,0,0.2);">
    <div style="font-size: 0.9rem; font-weight: 500; color: #a1a1aa; margin-bottom: 8px;">☁️ Secondary Engine (Cloud / OpenRouter) - Optional</div>
    <div style="font-size: 0.75rem; color: #71717a; margin-bottom: 12px;">Used exclusively for high-reasoning tasks like Critic and Writer to save VRAM and improve quality.</div>
    <div class="form-group">
      <label>Cloud API URL
        <input type="text" bind:value={appState.config.cloudApiUrl} placeholder="https://openrouter.ai/api/v1" />
      </label>
    </div>
    <div class="form-group" style="display:flex; gap: 8px;">
      <label style="flex:1;">Cloud API Key
        <input type="password" bind:value={appState.config.cloudLlmApiKey} placeholder="sk-or-v1-..." />
      </label>
      <label style="flex:1;">Cloud Model
        <input type="text" bind:value={appState.config.cloudModel} placeholder="anthropic/claude-3-opus-20240229" />
      </label>
    </div>
    <div style="margin-top: 16px; border-top: 1px solid #3f3f46; padding-top: 12px;">
      <div style="font-size: 0.85rem; font-weight: 500; color: #d4d4d8; margin-bottom: 8px;">Cloud Assigned Roles</div>
      <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px;">
        <label style="font-size: 0.8rem; display: flex; align-items: center; gap: 6px;">
          <input type="checkbox" bind:checked={appState.config.cloudRoutingCritic} /> {t("settings.role_critic")}
        </label>
        <label style="font-size: 0.8rem; display: flex; align-items: center; gap: 6px;">
          <input type="checkbox" bind:checked={appState.config.cloudRoutingWriter} /> {t("settings.role_writer")}
        </label>
        <label style="font-size: 0.8rem; display: flex; align-items: center; gap: 6px;">
          <input type="checkbox" bind:checked={appState.config.cloudRoutingPlanner} /> {t("settings.role_planner")}
        </label>
        <label style="font-size: 0.8rem; display: flex; align-items: center; gap: 6px;">
          <input type="checkbox" bind:checked={appState.config.cloudRoutingWorker} /> {t("settings.role_worker")}
        </label>
      </div>
    </div>
  </div>

  <div class="form-group">
    <label>{t("form.maxLoops")}
      <input type="number" min="1" max="10" bind:value={appState.config.maxLoops} />
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

  {#if appState.sysModalDomain === "setting_prompt"}
  <div class="form-group" style="display:flex; flex-direction:row; align-items:center; gap:8px; margin-bottom: 16px;">
    <input type="checkbox" id="multi-agent-toggle" bind:checked={appState.config.useMultiAgentWorkflow} style="width: auto;" />
    <label for="multi-agent-toggle" style="margin: 0; cursor: pointer;">{t("form.multiAgent")}</label>
  </div>
  <div class="form-group">
    <label>{t("form.systemPrompt")}
      <textarea bind:value={appState.config.systemPrompt} rows="4"></textarea>
    </label>
  </div>
  <hr style="border: 0; border-top: 1px solid #27272a; margin: 16px 0;" />
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">{t("form.plannerPrompt")} <button class="sys-badge" onclick={() => (appState.config.plannerPrompt = DEFAULT_PLANNER)}>[RESET]</button></label>
    <textarea bind:value={appState.config.plannerPrompt} rows="4"></textarea>
  </div>
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">{t("form.criticPrompt")} <button class="sys-badge" onclick={() => (appState.config.criticPrompt = DEFAULT_CRITIC)}>[RESET]</button></label>
    <textarea bind:value={appState.config.criticPrompt} rows="4"></textarea>
  </div>
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">{t("form.writerPrompt")} <button class="sys-badge" onclick={() => (appState.config.writerPrompt = DEFAULT_WRITER)}>[RESET]</button></label>
    <textarea bind:value={appState.config.writerPrompt} rows="3"></textarea>
  </div>
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">{t("form.reflectorPrompt")} <button class="sys-badge" onclick={() => (appState.config.reflectorPrompt = DEFAULT_REFLECTOR)}>[RESET]</button></label>
    <textarea bind:value={appState.config.reflectorPrompt} rows="3"></textarea>
  </div>
  <hr style="border: 0; border-top: 1px dashed #27272a; margin: 16px 0;" />
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">Background Worker Prompt <button class="sys-badge" onclick={() => (appState.config.workerPrompt = DEFAULT_WORKER)}>[RESET]</button></label>
    <textarea bind:value={appState.config.workerPrompt} rows="4"></textarea>
  </div>
  <div class="form-group">
    <label style="display:flex; justify-content:space-between;">Architecture Registry Agent <button class="sys-badge" onclick={() => (appState.config.registryPrompt = DEFAULT_REGISTRY)}>[RESET]</button></label>
    <textarea bind:value={appState.config.registryPrompt} rows="4"></textarea>
  </div>
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

  <div class="settings-footer" style="position: sticky; bottom: 0; background: #fcfbf8; padding-top: 12px; padding-bottom: 12px; margin-top: auto; border-top: 1px solid #ebebeb;">
    <button class="save-btn" onclick={saveSettings} style="width: 100%;">{t("form.saveBtn")}</button>
  </div>
</div>

<style>
  .settings-body {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    overflow-y: auto;
    flex: 1;
  }
  .form-group label {
    color: #555555;
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
  .settings-footer {
    padding: 20px 24px;
    border-top: 1px solid #1a1a1a;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    background: #f0ede1;
  }
  .save-btn {
    background: #1a1a1a;
    border: none;
    padding: 10px 20px;
    border-radius: 0;
    color: #fcfbf8;
    font-weight: 500;
    cursor: pointer;
    font-size: 0.95rem;
    text-transform: uppercase;
  }
  .save-btn:hover {
    background: #e0005a;
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
