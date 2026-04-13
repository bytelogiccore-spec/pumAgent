<script lang="ts">
  import { appState, interceptSlashCommand, addLog } from "../lib/store.svelte";
  import SettingsForm from "./SettingsForm.svelte";
  import KnowledgeManager from "./KnowledgeManager.svelte";
  import { t } from "../lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";

  async function onCloseClick() {
    try {
      await invoke("flush_db");
      addLog(t("sys.db_flush"));
    } catch (e: any) {
      console.warn("Failed to flush db:", e);
      addLog(t("sys.db_flush_err", { err: e }));
    }
    appState.sysModalOpen = false;
  }
</script>

<div class="settings-overlay">
  <div class="sys-modal">
    <div class="settings-header">
      <h2>{appState.sysModalTitle}</h2>
      <button class="close-btn" onclick={onCloseClick}>✕</button>
    </div>
    <div class="sys-body" style="display: flex; flex-direction: row; height: 100%;">
      <!-- 1. 좌측 메인 내비게이션 -->
      <div class="sys-nav" style="width: 200px; min-width: 200px; border-right: 1px solid #27272a; padding: 16px; display: flex; flex-direction: column; gap: 8px; background: #09090b; overflow-y: auto;">
        <div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-bottom: 4px;">{t("nav.label.system")}</div>
        <button class="sys-link" class:active={appState.sysModalDomain === "setting_server"} onclick={() => interceptSlashCommand("/setting_server")}>🌐 {t("nav.settings")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "setting_prompt"} onclick={() => interceptSlashCommand("/setting_prompt")}>{t("nav.setting_prompt")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "setting_heartbeat"} onclick={() => interceptSlashCommand("/setting_heartbeat")}>{t("nav.setting_heartbeat")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "setting_telegram"} onclick={() => interceptSlashCommand("/setting_telegram")}>{t("nav.setting_telegram")}</button>

        <div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-top: 16px; margin-bottom: 4px;">{t("nav.label.knowledge")}</div>
        <button class="sys-link" class:active={appState.sysModalDomain === "skills"} onclick={() => interceptSlashCommand("/skills")}>{t("nav.skills")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "rules"} onclick={() => interceptSlashCommand("/rules")}>{t("nav.rules")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "workflows"} onclick={() => interceptSlashCommand("/workflows")}>{t("nav.workflows")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "schedules"} onclick={() => interceptSlashCommand("/schedules")}>{t("nav.schedules")}</button>

        <div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-top: 16px; margin-bottom: 4px;">{t("nav.label.data")}</div>
        <button class="sys-link" class:active={appState.sysModalDomain === "brain"} onclick={() => interceptSlashCommand("/brain")}>{t("nav.brain")}</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "logs"} onclick={() => interceptSlashCommand("/logs")}>{t("nav.logs")}</button>

        <div style="font-size: 0.8rem; color: #a1a1aa; font-weight: bold; margin-top: 16px; margin-bottom: 4px;">SECURITY & SECRETS</div>
        <button class="sys-link" class:active={appState.sysModalDomain === "setting_terminal"} onclick={() => interceptSlashCommand("/setting_terminal")}>🛡️ Terminal Sandbox</button>
        <button class="sys-link" class:active={appState.sysModalDomain === "vault"} onclick={() => interceptSlashCommand("/vault")}>🔑 Vault (Keyring)</button>

        {#if appState.kbQuota}
        <div style="margin-top: auto; padding-top: 16px; display: flex; flex-direction: column; gap: 8px;">
          <div style="font-size: 0.75rem; color: #a1a1aa; font-weight: bold; text-transform: uppercase;">Context Quota</div>
          
          <!-- Rules Quota -->
          <div style="display: flex; flex-direction: column; gap: 2px;">
            <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: #d4d4d8;">
              <span>Rules</span>
              <span>{appState.kbQuota.rules.approx_tokens} / {appState.kbQuota.rules.limit}</span>
            </div>
            <div style="height: 4px; background: #27272a; border-radius: 2px; overflow: hidden;">
              <div style="height: 100%; background: {appState.kbQuota.rules.approx_tokens > appState.kbQuota.rules.limit * 0.8 ? '#e0005a' : '#0ea5e9'}; width: {Math.min(100, Math.max(0, (appState.kbQuota.rules.approx_tokens / appState.kbQuota.rules.limit) * 100))}%;"></div>
            </div>
          </div>

          <!-- Skills Quota -->
          <div style="display: flex; flex-direction: column; gap: 2px;">
            <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: #d4d4d8;">
              <span>Skills & Flow</span>
              <span>{(appState.kbQuota.skills.approx_tokens + appState.kbQuota.workflows.approx_tokens)} / {appState.kbQuota.skills.limit}</span>
            </div>
            <div style="height: 4px; background: #27272a; border-radius: 2px; overflow: hidden;">
              <div style="height: 100%; background: {(appState.kbQuota.skills.approx_tokens + appState.kbQuota.workflows.approx_tokens) > appState.kbQuota.skills.limit * 0.8 ? '#e0005a' : '#10b981'}; width: {Math.min(100, Math.max(0, ((appState.kbQuota.skills.approx_tokens + appState.kbQuota.workflows.approx_tokens) / appState.kbQuota.skills.limit) * 100))}%;"></div>
            </div>
          </div>
        </div>
        {/if}
      </div>

      <!-- 2. 우측 콘텐츠 영역 -->
      <div style="flex: 1; display: flex; flex-direction: row; overflow: hidden;">
        {#if appState.sysModalDomain.startsWith("setting_")}
          <SettingsForm />
        {:else}
          <KnowledgeManager />
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .settings-overlay {
    position: fixed;
    inset: 0;
    background: rgba(250, 249, 245, 0.9);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .sys-modal {
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    border-radius: 0;
    width: 1200px;
    max-width: 95vw;
    height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 12px 12px 0 0 rgba(0, 0, 0, 0.05);
  }
  .sys-body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .settings-header {
    padding: 12px 24px;
    border-bottom: 1px solid #1a1a1a;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .settings-header h2 {
    margin: 0;
    font-family: "Cormorant Garamond", serif;
    font-size: 1.4rem;
    font-weight: 600;
    color: #1a1a1a;
  }
  .close-btn {
    background: transparent;
    border: none;
    color: #1a1a1a;
    font-size: 1.5rem;
    cursor: pointer;
    line-height: 1;
  }
  .close-btn:hover {
    color: #e0005a;
  }
  .sys-nav::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.2);
  }
  .sys-nav::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.4);
  }

  .sys-nav .sys-link {
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
    color: #a1a1aa;
    padding: 8px 12px;
    border-radius: 6px;
    flex: none;
    width: 100%;
    transition: all 0.2s ease;
  }
  .sys-nav > * {
    flex-shrink: 0;
  }
  .sys-nav .sys-link:hover {
    background: #18181b;
    color: #fcfbf8;
  }
  .sys-nav .sys-link.active {
    background: #e0005a;
    color: #ffffff;
    font-weight: 600;
  }
</style>
