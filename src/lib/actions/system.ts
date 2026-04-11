import { invoke } from "@tauri-apps/api/core";
import { appState, addLog, showError } from "../store.svelte";
import { t } from "../i18n.svelte";

export async function interceptSlashCommand(cmd: string): Promise<boolean> {
  const parts = cmd.trim().split(" ");
  const command = parts[0].toLowerCase();

  if (command === "/settings" || command.startsWith("/setting_")) {
    appState.sysModalDomain = command === "/settings" ? "setting_server" : command.substring(1);
    appState.sysModalTitle = t("settings.title");
    appState.sysModalItems = [];
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = false;
    appState.selectedLogs = [];
    refreshKbQuota();
    return true;
  }

  if (command === "/brain") {
    appState.sysModalDomain = "brain";
    appState.sysModalTitle = t("nav.brain");
    try {
      let files: string[] = await invoke("list_brain_artifacts");
      appState.sysModalItems = files.map((f: string) => ({ name: f, content: null }));
    } catch (e) {
      appState.sysModalItems = [];
    }
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = true;
    appState.selectedLogs = [];
    refreshKbQuota();
    return true;
  }

  if (command === "/logs") {
    appState.sysModalDomain = "logs";
    appState.sysModalTitle = t("nav.logs");
    try {
      let files: string[] = await invoke("list_logs");
      appState.sysModalItems = files.map((f: string) => ({ name: f, content: null }));
    } catch (e) {
      appState.sysModalItems = [{ name: "Error", content: String(e) }];
    }
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = true;
    return true;
  }

  if (["/skills", "/rules", "/workflows", "/schedules"].includes(command)) {
    const domain = command.substring(1);
    appState.sysModalDomain = domain;
    appState.sysModalTitle = t("sys.knowledge_title", { domain: domain.toUpperCase() });
    try {
      let files: string[] = await invoke("list_knowledge", { domain });
      appState.sysModalItems = files.map((f: string) => ({ name: f, content: null }));
    } catch (e) {
      appState.sysModalItems = [];
    }
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = true;
    refreshKbQuota();
    return true;
  }

  return false;
}

export async function refreshKbQuota() {
  try {
    appState.kbQuota = await invoke("get_knowledge_quota");
  } catch (e) {
    console.warn("Failed to fetch KB quota", e);
  }
}

export async function loadSysItem(name: string) {
  try {
    if (appState.sysModalDomain === "logs") {
      appState.viewingItemContent = await invoke("read_log", { name });
    } else if (appState.sysModalDomain === "brain") {
      appState.viewingItemContent = await invoke("read_brain_artifact", { name });
    } else if (
      ["skills", "rules", "workflows", "schedules"].includes(appState.sysModalDomain)
    ) {
      appState.viewingItemContent = await invoke("read_knowledge", {
        domain: appState.sysModalDomain,
        name,
      });
    }
    appState.viewingItemName = name;
  } catch (e) {
    appState.viewingItemContent = `Error loading file: ${e}`;
  }
}

export async function deleteSysItem(name: string) {
  if (!confirm(t("sys.del_confirm", { name }))) return;
  try {
    if (appState.sysModalDomain === "brain") {
      await invoke("delete_brain_artifact", { name });
    } else {
      await invoke("delete_knowledge", { domain: appState.sysModalDomain, name });
    }
    appState.sysModalItems = appState.sysModalItems.filter((i) => i.name !== name);
    if (appState.viewingItemName === name) {
      appState.viewingItemContent = null;
      appState.viewingItemName = null;
    }
    refreshKbQuota();
    addLog(t("sys.del_done", { name }));
  } catch (e) {
    showError(`Delete failed: ${e}`);
  }
}

export async function deleteSelectedLogs() {
  if (appState.selectedLogs.length === 0) return;
  if (!confirm(t("sys.del_logs_confirm", { count: appState.selectedLogs.length }))) return;
  try {
    await invoke("delete_logs", { names: appState.selectedLogs });
    appState.sysModalItems = appState.sysModalItems.filter(i => !appState.selectedLogs.includes(i.name));
    if (appState.viewingItemName && appState.selectedLogs.includes(appState.viewingItemName)) {
      appState.viewingItemContent = null;
      appState.viewingItemName = null;
    }
    addLog(t("sys.del_logs_done", { count: appState.selectedLogs.length }));
    appState.selectedLogs = [];
  } catch (e) {
    showError(`Bulk delete failed: ${e}`);
  }
}

export async function saveSysItem() {
  if (!appState.viewingItemName) return;
  try {
    if (appState.sysModalDomain === "brain") {
      await invoke("write_brain_artifact", {
        name: appState.viewingItemName,
        content: appState.viewingItemContent || "",
      });
    } else {
      await invoke("write_knowledge", {
        domain: appState.sysModalDomain,
        name: appState.viewingItemName,
        content: appState.viewingItemContent || "",
      });
    }
    refreshKbQuota();
    addLog(t("sys.save_done", { name: appState.viewingItemName }));
    alert(t("sys.save_success"));
  } catch (e) {
    showError(`Save failed: ${e}`);
  }
}

export async function createSysItem() {
  let promptMsg = appState.sysModalDomain === "schedules" 
    ? t("sys.prompt_sched") 
    : t("sys.prompt_file");
  let name = prompt(promptMsg);
  if (!name) return;
  
  if (appState.sysModalDomain === "schedules") {
    if (!name.endsWith(".json")) {
       name = name.replace(/\.md$/, "") + ".json";
    }
  }

  let initialContent = "";
  if (appState.sysModalDomain === "schedules" && name.endsWith(".json")) {
      initialContent = `{\n  "name": "New Schedule",\n  "interval_seconds": 3600,\n  "description": "Description...",\n  "task_prompt": "Instruction...",\n  "last_run": null\n}`;
  }

  try {
    if (appState.sysModalDomain === "brain") {
      await invoke("write_brain_artifact", {
        name,
        content: initialContent,
      });
      let files: string[] = await invoke("list_brain_artifacts");
      appState.sysModalItems = files.map((f: string) => ({ name: f, content: null }));
    } else {
      await invoke("write_knowledge", {
        domain: appState.sysModalDomain,
        name,
        content: initialContent,
      });
      let files: string[] = await invoke("list_knowledge", {
        domain: appState.sysModalDomain,
      });
      appState.sysModalItems = files.map((f: string) => ({ name: f, content: null }));
    }
    appState.viewingItemName = name;
    appState.viewingItemContent = initialContent;
  } catch (e) {
    showError(`Create failed: ${e}`);
  }
}

export async function loadSettings() {
  try {
    const configData: any = await invoke("load_config");
    console.log("Loaded config:", configData);
    if (configData) {
      if (configData.endpoints) appState.config.endpoints = configData.endpoints;
      if (configData.planner_endpoint_id) appState.config.plannerEndpointId = configData.planner_endpoint_id;
      if (configData.critic_endpoint_id) appState.config.criticEndpointId = configData.critic_endpoint_id;
      if (configData.worker_endpoint_id) appState.config.workerEndpointId = configData.worker_endpoint_id;
      if (configData.reflector_endpoint_id) appState.config.reflectorEndpointId = configData.reflector_endpoint_id;
      if (configData.registry_endpoint_id) appState.config.registryEndpointId = configData.registry_endpoint_id;

      if (configData.max_loops) appState.config.maxLoops = configData.max_loops;
      if (configData.language) appState.config.language = configData.language;
      if (configData.system_prompt) appState.config.systemPrompt = configData.system_prompt;
      if (configData.search_provider) appState.config.searchProvider = configData.search_provider;
      if (configData.tavily_api_key) appState.config.tavilyApiKey = configData.tavily_api_key;
      if (configData.google_api_key) appState.config.googleApiKey = configData.google_api_key;
      if (configData.google_cx) appState.config.googleCx = configData.google_cx;
      
      if (configData.use_multi_agent_workflow !== undefined) appState.config.useMultiAgentWorkflow = configData.use_multi_agent_workflow;
      if (configData.use_think_mode !== undefined) appState.config.useThinkMode = configData.use_think_mode;
      if (configData.planner_prompt) appState.config.plannerPrompt = configData.planner_prompt;
      if (configData.critic_prompt) appState.config.criticPrompt = configData.critic_prompt;
      if (configData.writer_prompt) appState.config.writerPrompt = configData.writer_prompt;
      if (configData.reflector_prompt) appState.config.reflectorPrompt = configData.reflector_prompt;
      if (configData.heartbeat_prompt) appState.config.heartbeatPrompt = configData.heartbeat_prompt;
      if (configData.worker_prompt) appState.config.workerPrompt = configData.worker_prompt;
      if (configData.registry_prompt) appState.config.registryPrompt = configData.registry_prompt;
      
      if (configData.heartbeat_enabled !== undefined) appState.config.heartbeatEnabled = configData.heartbeat_enabled;
      if (configData.heartbeat_interval) appState.config.heartbeatInterval = configData.heartbeat_interval;
      
      if (configData.telegram_enabled !== undefined) appState.config.telegramEnabled = configData.telegram_enabled;
      if (configData.telegram_bot_token) appState.config.telegramBotToken = configData.telegram_bot_token;
      if (configData.telegram_chat_id) appState.config.telegramChatId = configData.telegram_chat_id;

      if (configData.kb_rules_token_limit !== undefined) appState.config.kbRulesTokenLimit = configData.kb_rules_token_limit;
      if (configData.kb_skills_token_limit !== undefined) appState.config.kbSkillsTokenLimit = configData.kb_skills_token_limit;

      appState.config.isFirstRun = false;
      addLog(t("sys.config_loaded"));
    }
  } catch (err) {
    console.error("No valid local config found, using defaults", err);
  }
}

export async function saveSettings(closeModal: boolean = true) {
  try {
    await invoke("save_config", {
      config: {
        is_first_run: appState.config.isFirstRun,
        endpoints: appState.config.endpoints,
        planner_endpoint_id: appState.config.plannerEndpointId,
        writer_endpoint_id: appState.config.writerEndpointId,
        worker_endpoint_id: appState.config.workerEndpointId,
        reflector_endpoint_id: appState.config.reflectorEndpointId,
        registry_endpoint_id: appState.config.registryEndpointId,
        max_loops: appState.config.maxLoops,
        language: appState.config.language,
        system_prompt: appState.config.systemPrompt,
        search_provider: appState.config.searchProvider,
        tavily_api_key: appState.config.tavilyApiKey,
        google_api_key: appState.config.googleApiKey,
        google_cx: appState.config.googleCx,
        use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
        use_think_mode: appState.config.useThinkMode,
        planner_prompt: appState.config.plannerPrompt,
        critic_prompt: appState.config.criticPrompt,
        writer_prompt: appState.config.writerPrompt,
        reflector_prompt: appState.config.reflectorPrompt,
        heartbeat_prompt: appState.config.heartbeatPrompt,
        worker_prompt: appState.config.workerPrompt,
        registry_prompt: appState.config.registryPrompt,
        heartbeat_enabled: appState.config.heartbeatEnabled,
        heartbeat_interval: appState.config.heartbeatInterval,
        telegram_enabled: appState.config.telegramEnabled,
        telegram_bot_token: appState.config.telegramBotToken,
        telegram_chat_id: appState.config.telegramChatId,
        kb_rules_token_limit: appState.config.kbRulesTokenLimit,
        kb_skills_token_limit: appState.config.kbSkillsTokenLimit,
      },
    });
    await invoke("flush_db");
    if (closeModal) {
      appState.sysModalOpen = false;
    }
    addLog(t("sys.config_saved"));
    addLog(t("sys.db_flush"));
  } catch (err: any) {
    addLog(t("settings.save_err", { err }));
  }
}
