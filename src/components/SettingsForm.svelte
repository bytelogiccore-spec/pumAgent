<script lang="ts">
  import { appState, saveSettings } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";

  import SettingsRouting from "./settings/SettingsRouting.svelte";
  import SettingsPrompts from "./settings/SettingsPrompts.svelte";
  import SettingsSystem from "./settings/SettingsSystem.svelte";
  import SettingsTerminal from "./settings/SettingsTerminal.svelte";
</script>

<div class="settings-body" style="width: 100%; height: 100%; overflow-y: auto; position: relative;">
  
  {#if appState.sysModalDomain === "setting_server"}
    <SettingsSystem />
    <SettingsRouting />
  {/if}

  {#if appState.sysModalDomain === "setting_prompt"}
    <SettingsPrompts />
  {/if}

  {#if appState.sysModalDomain === "setting_heartbeat" || appState.sysModalDomain === "setting_telegram"}
    <SettingsSystem />
  {/if}

  {#if appState.sysModalDomain === "setting_terminal"}
    <SettingsTerminal />
  {/if}

  <div class="settings-footer">
    <button class="save-btn" onclick={() => saveSettings(true)}>{t("form.saveBtn")}</button>
  </div>
</div>

<style>
  .settings-body {
    box-sizing: border-box;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    overflow-y: auto;
    flex: 1;
  }
  .settings-footer {
    position: sticky;
    bottom: -24px;
    margin-left: -24px;
    margin-right: -24px;
    margin-bottom: -24px;
    margin-top: auto;
    z-index: 10;
    padding: 16px 24px;
    border-top: 1px solid #1a1a1a;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    background: #f0ede1;
  }
  .save-btn {
    width: 100%;
    background: #1a1a1a;
    border: none;
    padding: 12px 20px;
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
</style>
