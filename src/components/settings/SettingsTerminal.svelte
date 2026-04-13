<script lang="ts">
  import { appState } from "../../lib/store.svelte";
</script>

<div class="form-group">
  <label>Terminal Sandbox Blocklist
    <textarea bind:value={appState.config.terminalBlocklistCsv} rows="6" placeholder="e.g. format, rm -rf, mkfs, sudo" style="margin-top: 8px;"></textarea>
  </label>
</div>

<div class="info-block" style="margin-top: 16px; padding: 12px; background: #27272a; border-radius: 6px; color: #d4d4d8; font-size: 0.85rem;">
  <p style="margin:0 0 8px 0;"><strong>Security Note:</strong></p>
  <ul style="margin:0; padding-left: 20px;">
    <li>You can add custom dangerous commands that the AI Terminal Tool should be blocked from executing.</li>
    <li>Separate keywords with commas (e.g. <code>shutdown, diskpart, del</code>).</li>
    <li>OS default blocks (like <code>rm -rf /</code>, <code>format C:</code>) are ALWAYS enforced natively in Rust.</li>
  </ul>
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
    resize: none;
  }
  .form-group textarea:focus {
    outline: none;
    border-color: #e0005a;
  }
</style>
