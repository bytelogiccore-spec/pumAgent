<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import SystemModal from "./components/SystemModal.svelte";
  import LogPanel from "./components/LogPanel.svelte";
  import ChatSection from "./components/ChatSection.svelte";
  import StartupWizard from "./components/StartupWizard.svelte";
  import ErrorPopup from "./components/ErrorPopup.svelte";
  import { appState, addLog, triggerHeartbeat } from "./lib/store.svelte";
  import { initLocales, t } from "./lib/i18n.svelte";

  $effect(() => {
    if (appState.pendingHeartbeat && !appState.isThinking && appState.query.trim().length === 0 && appState.pendingUserQueries.length === 0) {
      appState.pendingHeartbeat = false;
      setTimeout(() => {
        if (!appState.isThinking && appState.query.trim().length === 0) {
          triggerHeartbeat();
        }
      }, 300);
    }
  });

  onMount(async () => {
    listen<string>("tool_log", (event) => {
      addLog(event.payload);
    });

    listen<string>("system_panic", (event) => {
      appState.globalError = event.payload;
    });

    listen<number>("heartbeat_progress", (event) => {
      appState.heartbeatRemainingSec = event.payload;
    });

    listen("heartbeat_tick", () => {
      if (appState.isThinking || appState.query.trim().length > 0 || appState.pendingUserQueries.length > 0) {
        appState.pendingHeartbeat = true;
        return;
      }
      triggerHeartbeat();
    });

    try {
      let loadedConfig: any = await invoke("load_config");
      appState.config = { ...appState.config, ...loadedConfig };
      
      // Override specific mappings
      appState.config.isFirstRun = loadedConfig.is_first_run ?? true;
      appState.config.language = loadedConfig.language || "en";
      appState.config.maxLoops = loadedConfig.max_loops || 3;
      appState.config.systemPrompt = loadedConfig.system_prompt || appState.config.systemPrompt;
      appState.config.searchProvider = loadedConfig.search_provider || "duckduckgo";
      appState.config.tavilyApiKey = loadedConfig.tavily_api_key || "";
      appState.config.googleApiKey = loadedConfig.google_api_key || "";
      appState.config.googleCx = loadedConfig.google_cx || "";
      appState.config.useMultiAgentWorkflow = loadedConfig.use_multi_agent_workflow || false;
      appState.config.useThinkMode = loadedConfig.use_think_mode ?? true;
      appState.config.plannerPrompt = loadedConfig.planner_prompt || appState.config.plannerPrompt;
      appState.config.criticPrompt = loadedConfig.critic_prompt || appState.config.criticPrompt;
      appState.config.writerPrompt = loadedConfig.writer_prompt || appState.config.writerPrompt;
      appState.config.reflectorPrompt = loadedConfig.reflector_prompt || appState.config.reflectorPrompt;
      appState.config.heartbeatPrompt = loadedConfig.heartbeat_prompt || appState.config.heartbeatPrompt;
      appState.config.workerPrompt = loadedConfig.worker_prompt || appState.config.workerPrompt;
      appState.config.registryPrompt = loadedConfig.registry_prompt || appState.config.registryPrompt;
      appState.config.heartbeatEnabled = loadedConfig.heartbeat_enabled || false;
      appState.config.heartbeatInterval = loadedConfig.heartbeat_interval || 3600;
      appState.config.telegramEnabled = loadedConfig.telegram_enabled || false;
      appState.config.telegramBotToken = loadedConfig.telegram_bot_token || "";
      appState.config.telegramChatId = loadedConfig.telegram_chat_id || "";

      // Initialize Locales
      await initLocales();

      // Update the welcome message to match the loaded language
      if (appState.messages.length > 0 && appState.messages[0].role === "assistant") {
        appState.messages[0].content = t("sys.welcome");
      }

      addLog(t("sys.config_loaded"));
    } catch (err) {
      console.warn("Failed to load config:", err);
    }
  });
</script>

<main class="app-container">
  <ErrorPopup />
  {#if appState.config.isFirstRun}
    <StartupWizard />
  {:else if appState.sysModalOpen}
    <SystemModal />
  {/if}

  <div class="main-layout" class:blur-bg={appState.config.isFirstRun || appState.sysModalOpen}>
    <ChatSection />
    
    {#if appState.logExpanded}
      <LogPanel />
    {/if}
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: #e5e5e5;
    font-family:
      "Inter",
      -apple-system,
      BlinkMacSystemFont,
      "Segoe UI",
      Roboto,
      Helvetica,
      Arial,
      sans-serif;
    color: #1a1a1a;
  }
  .app-container {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .main-layout {
    flex: 1;
    display: flex;
    overflow: hidden;
    transition: filter 0.3s;
  }
  .blur-bg {
    filter: blur(4px);
    pointer-events: none;
  }
</style>
