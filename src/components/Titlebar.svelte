<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { appState } from "../lib/store.svelte";
  import { onMount } from "svelte";
  
  let appWindow: any;
  let appVersion = $state("");

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch (e) {}
  });
  try {
    appWindow = getCurrentWindow();
  } catch (err) {
    appState.globalError = "Window init error: " + err;
  }

  async function minimize() {
    try { await appWindow.minimize(); } catch(e) { appState.globalError = "Minimize err: " + e; }
  }

  async function toggleMaximize() {
    try { await appWindow.toggleMaximize(); } catch(e) { appState.globalError = "Maximize err: " + e; }
  }

  async function close() {
    try { await appWindow.close(); } catch(e) { appState.globalError = "Close err: " + e; }
  }
</script>

<div data-tauri-drag-region class="titlebar">
  <div data-tauri-drag-region class="title-section">
    <!-- Optional: you can include the logo here -->
    <div data-tauri-drag-region class="logo-container">
      <img src="/favicon.png" alt="logo" class="logo" />
    </div>
    <span data-tauri-drag-region class="title-text">PumAgent</span>
    {#if appVersion}
      <span data-tauri-drag-region class="version-text">v{appVersion}</span>
    {/if}
  </div>
  
  <div class="window-controls">
    <div class="control-btn minimize" onclick={minimize} onkeydown={(e) => e.key === 'Enter' && minimize()} tabindex="0" role="button">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor">
        <path d="M2.5 6.5h7" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </div>
    <div class="control-btn maximize" onclick={toggleMaximize} onkeydown={(e) => e.key === 'Enter' && toggleMaximize()} tabindex="0" role="button">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor">
        <rect x="2.5" y="2.5" width="7" height="7" stroke-width="1.2" rx="1" />
      </svg>
    </div>
    <div class="control-btn close" onclick={close} onkeydown={(e) => e.key === 'Enter' && close()} tabindex="0" role="button">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor">
        <path d="M3 3l6 6M9 3L3 9" stroke-width="1.2" stroke-linecap="round" />
      </svg>
    </div>
  </div>
</div>

<style>
  .titlebar {
    height: 38px;
    background: rgba(22, 22, 24, 0.95);
    backdrop-filter: blur(10px);
    display: flex;
    justify-content: space-between;
    align-items: center;
    user-select: none;
    -webkit-user-select: none;
    box-shadow: 0 1px 3px rgba(0,0,0,0.3);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    z-index: 10000;
  }

  .title-section {
    display: flex;
    align-items: center;
    padding-left: 12px;
    height: 100%;
    /* Flex-grow to allow dragging on the empty middle space */
    flex-grow: 1; 
  }

  .logo-container {
    display: flex;
    align-items: center;
    justify-content: center;
    margin-right: 10px;
  }

  .logo {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    filter: drop-shadow(0 0 3px rgba(0, 195, 255, 0.5));
  }

  .title-text {
    font-size: 13px;
    font-weight: 500;
    color: #e0e0e0;
    letter-spacing: 0.5px;
  }

  .version-text {
    font-size: 11px;
    color: #888;
    margin-left: 8px;
    font-family: monospace;
    margin-top: 1px;
  }

  .window-controls {
    display: flex;
    height: 100%;
  }

  .control-btn {
    width: 44px;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    color: #a0a0a0;
    transition: all 0.2s ease;
    cursor: default;
    -webkit-app-region: no-drag;
  }

  .control-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .control-btn:active {
    background: rgba(255, 255, 255, 0.15);
  }

  .control-btn.close:hover {
    background: #e81123;
    color: #fff;
  }

  .control-btn.close:active {
    background: #c10e1c;
  }

  /* Make sure the drag region works everywhere it should */
  [data-tauri-drag-region] {
    cursor: default;
    -webkit-app-region: drag;
  }
</style>
