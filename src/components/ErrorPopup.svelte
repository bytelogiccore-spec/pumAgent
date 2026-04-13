<script lang="ts">
  import { appState } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";

  let showCopySuccess = $state(false);

  function closePopup() {
    appState.globalError = null;
  }

  async function copyError() {
    if (appState.globalError) {
      try {
        await navigator.clipboard.writeText(appState.globalError);
        showCopySuccess = true;
        setTimeout(() => (showCopySuccess = false), 2000);
      } catch (e) {
        console.error("Clipboard copy failed:", e);
      }
    }
  }
</script>

{#if appState.globalError}
  <div class="error-overlay" onclick={closePopup} role="button" tabindex="0" onkeypress={(e) => e.key === 'Enter' && closePopup()}>
    <div class="error-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="error-header">
        <div class="error-title">
          <span class="icon">⚠️</span> Error Occurred
        </div>
        <button class="close-btn" onclick={closePopup}>✕</button>
      </div>

      <div class="error-body">
        <textarea class="error-textarea" readonly value={appState.globalError}></textarea>
      </div>

      <div class="error-footer">
        <button class="copy-btn" onclick={copyError}>
          {#if showCopySuccess}
            ✅ Copied!
          {:else}
            📋 Copy Error
          {/if}
        </button>
        <button class="ok-btn" onclick={closePopup}>OK</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .error-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    padding: 20px;
    animation: fadeIn 0.15s ease-out;
  }

  .error-modal {
    background: #fcfbf8;
    border: 1px solid #e0005a;
    border-radius: 8px;
    width: 100%;
    max-width: 500px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 25px rgba(224, 0, 90, 0.2);
    animation: slideUp 0.2s ease-out;
    overflow: hidden;
  }

  .error-header {
    background: #ffe5ec;
    color: #e0005a;
    padding: 16px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid #ffccd9;
    font-weight: bold;
    font-size: 1.1rem;
  }

  .error-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: #e0005a;
    font-size: 1.2rem;
    cursor: pointer;
    opacity: 0.7;
    transition: 0.2s;
  }

  .close-btn:hover {
    opacity: 1;
    transform: scale(1.1);
  }

  .error-body {
    padding: 20px;
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .error-textarea {
    width: 100%;
    min-height: 150px;
    border: 1px solid #ddd;
    background: #fdfdfd;
    padding: 12px;
    font-family: monospace;
    font-size: 0.85rem;
    color: #333;
    border-radius: 4px;
    resize: none;
    outline: none;
  }

  .error-textarea:focus {
    border-color: #e0005a;
  }

  .error-footer {
    padding: 16px 20px;
    background: #f5f3ed;
    border-top: 1px solid #ebebeb;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
  }

  .copy-btn {
    background: #fff;
    border: 1px solid #1a1a1a;
    color: #1a1a1a;
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: 0.2s;
  }

  .copy-btn:hover {
    background: #f0f0f0;
  }

  .ok-btn {
    background: #e0005a;
    border: 1px solid #e0005a;
    color: #fff;
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
    font-weight: bold;
    transition: 0.2s;
  }

  .ok-btn:hover {
    background: #c3004e;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(20px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
