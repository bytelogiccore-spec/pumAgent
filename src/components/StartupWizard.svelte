<script lang="ts">
  import { appState, saveSettings } from "../lib/store.svelte";
  import { initLocales } from "../lib/i18n.svelte";

  const presets = [
    { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1/chat/completions", model: "gpt-4o" },
    { id: "groq", name: "Groq (Fast Open Source)", url: "https://api.groq.com/openai/v1/chat/completions", model: "mixtral-8x7b-32768" },
    { id: "together", name: "Together.ai", url: "https://api.together.xyz/v1/chat/completions", model: "meta-llama/Llama-3-70b-chat-hf" },
    { id: "local", name: "Local Server (LM Studio / Ollama)", url: "http://127.0.0.1:1234/v1/chat/completions", model: "local-model" }
  ];

  let selectedPreset = $state(presets[0]);
  let apiKey = $state("");

  async function nextStep() {
    if (appState.config.endpoints && appState.config.endpoints.length > 0) {
      appState.config.endpoints[0].api_url = selectedPreset.url;
      appState.config.endpoints[0].model = selectedPreset.model;
      appState.config.endpoints[0].api_key = apiKey;
    }
    appState.config.language = "en";
    appState.config.isFirstRun = false;
    await saveSettings();
  }
</script>

<div class="wizard-overlay">
  <div class="wizard-box">
    <h2>Startup Wizard</h2>
    
    <div class="step-content">
      <h3>AI Provider Setup</h3>
      <p>PumAgent requires an OpenAI-compatible API endpoint.</p>
      <select bind:value={selectedPreset} class="sys-select">
        {#each presets as preset}
          <option value={preset}>{preset.name}</option>
        {/each}
      </select>
      
      {#if selectedPreset.id === 'local'}
        <div style="margin-top: 12px;">
          <label for="apiUrl">Local API Endpoint URL</label>
          <input id="apiUrl" type="text" bind:value={selectedPreset.url} class="sys-select" />
        </div>
        <div style="margin-top: 12px;">
          <label for="modelId">Local Model ID</label>
          <input id="modelId" type="text" bind:value={selectedPreset.model} class="sys-select" />
        </div>
      {/if}
      
      <div style="margin-top: 12px;">
        <label for="apiKey">API Key {selectedPreset.id === 'local' ? '(Optional)' : ''}</label>
        <input id="apiKey" type="password" bind:value={apiKey} class="sys-select" placeholder="sk-..." />
      </div>
    </div>

    <div class="actions">
      <button onclick={nextStep} class="save-btn">
        Start PumAgent
      </button>
    </div>
  </div>
</div>

<style>
  .wizard-overlay {
    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background: #f5f3ed; z-index: 9999;
    display: flex; justify-content: center; align-items: center;
  }
  .wizard-box {
    background: #fff; border: 1px solid #1a1a1a;
    padding: 32px; box-shadow: 8px 8px 0px rgba(0,0,0,1);
    width: 400px; max-width: 90vw;
  }
  .wizard-box h2 {
    margin-top: 0; border-bottom: 2px solid #1a1a1a; padding-bottom: 8px;
  }
  .step-content {
    margin: 20px 0;
  }
  .actions {
    display: flex; justify-content: flex-end; margin-top: 24px;
  }
  .sys-select {
    width: 100%; box-sizing: border-box; padding: 10px;
    border: 1px solid #1a1a1a; background: #fcfbf8;
  }
  .save-btn {
    background: #1a1a1a; color: #fff; border: none;
    padding: 10px 20px; font-weight: 600; cursor: pointer; text-transform: uppercase;
  }
  .save-btn:hover { background: #e0005a; }
</style>
