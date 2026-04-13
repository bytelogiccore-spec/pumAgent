<script lang="ts">
  import { appState } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { DEFAULT_PLANNER, DEFAULT_CRITIC, DEFAULT_WRITER, DEFAULT_REFLECTOR, DEFAULT_WORKER, DEFAULT_REGISTRY } from "../../lib/constants";

  let activeTab = $state('planner');
  const tabs = [
    { id: 'planner', label: 'Planner' },
    { id: 'critic', label: 'Critic' },
    { id: 'writer', label: 'Writer' },
    { id: 'reflector', label: 'Reflector' },
    { id: 'worker', label: 'Worker' },
    { id: 'registry', label: 'Registry' }
  ];
</script>

<div class="form-group" style="display:flex; flex-direction:row; align-items:center; gap:8px; margin-bottom: 16px; padding-bottom: 16px; border-bottom: 1px solid #27272a;">
  <input type="checkbox" id="multi-agent-toggle" bind:checked={appState.config.useMultiAgentWorkflow} style="width: auto;" />
  <label for="multi-agent-toggle" style="margin: 0; cursor: pointer;">{t("form.multiAgent") || "Enable Multi-Agent Workflow"}</label>
</div>

{#if !appState.config.useMultiAgentWorkflow}
  <!-- Single Agent Mode -->
  <div class="form-group">
    <label>{t("form.systemPrompt") || "Main Agent System Prompt"}
      <textarea bind:value={appState.config.systemPrompt} rows="15"></textarea>
    </label>
  </div>
{:else}
  <!-- Multi-Agent Mode -->
  <div class="tabs-container">
    {#each tabs as tab}
      <button class="sys-tab {activeTab === tab.id ? 'active' : ''}" onclick={() => activeTab = tab.id}>
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="tab-content">
    {#if activeTab === 'planner'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.plannerPrompt")} <button class="sys-badge" onclick={() => (appState.config.plannerPrompt = DEFAULT_PLANNER)}>[RESET]</button></label>
        <textarea bind:value={appState.config.plannerPrompt} rows="15"></textarea>
      </div>
    {:else if activeTab === 'critic'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.criticPrompt")} <button class="sys-badge" onclick={() => (appState.config.criticPrompt = DEFAULT_CRITIC)}>[RESET]</button></label>
        <textarea bind:value={appState.config.criticPrompt} rows="15"></textarea>
      </div>
    {:else if activeTab === 'writer'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.writerPrompt")} <button class="sys-badge" onclick={() => (appState.config.writerPrompt = DEFAULT_WRITER)}>[RESET]</button></label>
        <textarea bind:value={appState.config.writerPrompt} rows="15"></textarea>
      </div>
    {:else if activeTab === 'reflector'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.reflectorPrompt")} <button class="sys-badge" onclick={() => (appState.config.reflectorPrompt = DEFAULT_REFLECTOR)}>[RESET]</button></label>
        <textarea bind:value={appState.config.reflectorPrompt} rows="15"></textarea>
      </div>
    {:else if activeTab === 'worker'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.workerPrompt") || "Background Worker Prompt"} <button class="sys-badge" onclick={() => (appState.config.workerPrompt = DEFAULT_WORKER)}>[RESET]</button></label>
        <textarea bind:value={appState.config.workerPrompt} rows="15"></textarea>
      </div>
    {:else if activeTab === 'registry'}
      <div class="form-group">
        <label style="display:flex; justify-content:space-between;">{t("form.registryPrompt") || "Architecture Registry Agent"} <button class="sys-badge" onclick={() => (appState.config.registryPrompt = DEFAULT_REGISTRY)}>[RESET]</button></label>
        <textarea bind:value={appState.config.registryPrompt} rows="15"></textarea>
      </div>
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
  .form-group textarea {
    width: 100%;
    box-sizing: border-box;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 12px;
    border-radius: 0;
    color: #1a1a1a;
    font-family: inherit;
    font-size: 1rem;
    resize: vertical;
  }
  .form-group input:focus,
  .form-group textarea:focus {
    outline: none;
    border-color: #e0005a;
  }
  .sys-badge {
    flex-shrink: 0;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 2px 8px;
    border-radius: 0;
    color: #1a1a1a;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: 0.2s;
  }
  .sys-badge:hover {
    background: #e0005a;
    color: #fcfbf8;
    border-color: #e0005a;
  }
  .tabs-container {
    display: flex;
    gap: 4px;
    border-bottom: 2px solid rgba(255, 255, 255, 0.1);
    margin-bottom: 16px;
    overflow-x: auto;
  }
  .sys-tab {
    background: transparent;
    border: none;
    color: #a1a1aa;
    padding: 8px 16px;
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    margin-bottom: -2px;
    transition: all 0.2s;
  }
  .sys-tab:hover {
    color: #e4e4e7;
  }
  .sys-tab.active {
    color: #10b981;
    border-bottom-color: #10b981;
  }
  .tab-content {
    animation: fadeIn 0.2s ease-out;
  }
  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
