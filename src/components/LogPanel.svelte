<script lang="ts">
  import { appState } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { tick } from "svelte";

  let logBoxEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    let _ = appState.logs.length;
    tick().then(() => {
      if (logBoxEl) {
        logBoxEl.scrollTop = logBoxEl.scrollHeight;
      }
    });
  });
</script>

<div class="log-section">
  <div class="log-header">
    <h2>{t("log.title")}</h2>
    <button class="clear-log-btn" onclick={() => (appState.logs = [])} title={t("log.clear")}>{t("log.clear")}</button>
  </div>
  <div class="log-box" bind:this={logBoxEl}>
    {#if appState.logs.length === 0}
      <div class="empty-log">{t("log.empty")}</div>
    {:else}
      {#each appState.logs as logItem}
        <div class="log-item">{logItem}</div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .log-section {
    width: 35vw;
    border-left: 1px solid #1a1a1a;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    background: #ebe8de;
    transition: width 0.3s ease-out;
  }
  .log-header {
    padding: 16px 28px;
    background: #ebe8de;
    border-bottom: 1px solid #1a1a1a;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .log-header h2 {
    font-family: "Cormorant Garamond", Georgia, serif;
    font-size: 1.1rem;
    color: #1a1a1a;
    margin: 0;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .clear-log-btn {
    background: transparent;
    border: none;
    color: #e0005a;
    font-family: "Inter", sans-serif;
    font-size: 0.8rem;
    text-transform: uppercase;
    cursor: pointer;
    transition: 0.2s;
  }
  .clear-log-btn:hover {
    text-decoration: underline;
  }
  .log-box {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 0.85rem;
  }
  .log-item {
    color: #333333;
    padding: 8px 0;
    border-bottom: 1px dashed #cccccc;
    line-height: 1.6;
    word-wrap: break-word;
  }
  .empty-log {
    color: #777777;
    text-align: left;
    margin-top: 20px;
    font-style: italic;
  }
</style>
