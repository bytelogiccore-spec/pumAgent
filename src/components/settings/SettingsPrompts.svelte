<script lang="ts">
  import { appState } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { DEFAULT_PLANNER, DEFAULT_CRITIC, DEFAULT_WRITER, DEFAULT_REFLECTOR, DEFAULT_WORKER, DEFAULT_REGISTRY } from "../../lib/constants";
</script>

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
</style>
