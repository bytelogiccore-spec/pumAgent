import { invoke } from "@tauri-apps/api/core";
import { appState, addLog, showError } from "../store.svelte";
import { t } from "../i18n.svelte";
import { interceptSlashCommand } from "./system";

export function wipeChat() {
  if (!confirm(t("sys.clear_chat_confirm"))) return;
  appState.messages = [
    {
      role: "assistant",
      content: "",
      logs: [],
    },
  ];
  appState.logs = [];
  addLog(t("sys.clear_chat_done"));
  appState.messages[0].content = t("chat.hello");
}

export async function compressChatMemory(silent: boolean = false) {
  if (appState.messages.length <= 4) {
    if (!silent) showError(t("sys.compress_error_count"));
    return;
  }
  
  if (!silent && !confirm(t("sys.compress_confirm"))) return;

  appState.isThinking = true;
  addLog(t("sys.compress_start"));
  
  try {
    let recentCount = 2; // retain very last user + assistant
    let oldMessages = appState.messages.slice(0, appState.messages.length - recentCount);
    let recentMessages = appState.messages.slice(appState.messages.length - recentCount);

    let payload = {
      endpoints: appState.config.endpoints,
      messages: oldMessages,
    };

    let summary: string = await invoke("compress_memory", { payload });
    
    appState.messages = [
      {
        role: "assistant",
        content: t("sys.compress_summary", { summary })
      },
      ...recentMessages
    ];
    
    addLog(t("sys.compress_done", { count: oldMessages.length }));
  } catch (err: any) {
    addLog(t("sys.compress_error", { err }));
    showError(t("sys.compress_error_alert", { err }));
  } finally {
    appState.isThinking = false;
    processPendingQueues();
  }
}

export async function stopAgent() {
  try {
    await invoke("stop_agent");
    addLog(t("sys.cancel_req"));
  } catch (e) {
    showError(`Stop failed: ${e}`);
  }
}

export async function triggerHeartbeat() {
  try {
    let payload = {
      session_id: null,
      endpoints: appState.config.endpoints,
      planner_endpoint_id: appState.config.plannerEndpointId,
      critic_endpoint_id: appState.config.criticEndpointId,
      writer_endpoint_id: appState.config.writerEndpointId,
      worker_endpoint_id: appState.config.workerEndpointId,
      reflector_endpoint_id: appState.config.reflectorEndpointId,
      registry_endpoint_id: appState.config.registryEndpointId,
      system_prompt: appState.config.systemPrompt,
      planner_prompt: appState.config.plannerPrompt,
      critic_prompt: appState.config.criticPrompt,
      writer_prompt: appState.config.writerPrompt,
      reflector_prompt: appState.config.reflectorPrompt,
      max_loops: appState.config.maxLoops,
      use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
      use_think_mode: appState.config.useThinkMode,
      language: appState.config.language,
      worker_prompt: appState.config.workerPrompt,
      registry_prompt: appState.config.registryPrompt,
    };

    let result: string = await invoke("execute_background_scheduler", { payload });

    if (result === "No tasks") {
      // Memory compression during idle heartbeat (Prevent premature context loss)
      if (appState.messages.length > 20) {
        addLog(t("sys.hb_compress", { length: appState.messages.length }));
        await compressChatMemory(true);
      }
    } else {
      addLog(t("sys.hb_start"));
    }
  } catch (err: any) {
    addLog(t("sys.hb_err", { err }));
  } finally {
    processPendingQueues();
  }
}

export async function internalExecuteAgent() {
  try {
    let payload = {
      session_id: appState.sessionId,
      endpoints: appState.config.endpoints,
      planner_endpoint_id: appState.config.plannerEndpointId,
      critic_endpoint_id: appState.config.criticEndpointId,
      writer_endpoint_id: appState.config.writerEndpointId,
      worker_endpoint_id: appState.config.workerEndpointId,
      reflector_endpoint_id: appState.config.reflectorEndpointId,
      registry_endpoint_id: appState.config.registryEndpointId,
      system_prompt: appState.config.systemPrompt,
      planner_prompt: appState.config.plannerPrompt,
      critic_prompt: appState.config.criticPrompt,
      writer_prompt: appState.config.writerPrompt,
      reflector_prompt: appState.config.reflectorPrompt,
      max_loops: appState.config.maxLoops,
      use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
      use_think_mode: appState.config.useThinkMode,
      language: appState.config.language,
      worker_prompt: appState.config.workerPrompt,
      registry_prompt: appState.config.registryPrompt,
      messages: [...appState.messages]
        .filter((m) => m.role === "user" || m.role === "assistant")
        .filter((m, i, arr) => !(i === arr.length - 1 && m.role === "assistant" && !m.content)),
    };

    let results: any = await invoke("execute_agent_tools", { payload });
    let lastIndex = appState.messages.length - 1;
    let lastMsg = appState.messages[lastIndex];
    lastMsg.content = results.final_output;
    appState.messages[lastIndex] = lastMsg;
  } catch (err: any) {
    addLog(t("sys.bridge_err", { err }));
    let lastIndex = appState.messages.length - 1;
    let lastMsg = appState.messages[lastIndex];
    lastMsg.content = t("sys.bridge_err_msg", { err });
    appState.messages[lastIndex] = lastMsg;
  } finally {
    appState.isThinking = false;
    if (appState.timerInterval !== null) {
      clearInterval(appState.timerInterval);
    }
    processPendingQueues();
  }
}

export function processPendingQueues() {
  if (appState.pendingUserQueries.length > 0) {
    appState.query = appState.pendingUserQueries.shift()!;
    setTimeout(() => submitQuery(), 100);
  } else if (appState.pendingHeartbeat && appState.query.trim().length === 0) {
    appState.pendingHeartbeat = false;
    setTimeout(() => triggerHeartbeat(), 100);
  }
}

export async function submitQuery() {
  if (!appState.query.trim()) return;
  if (appState.query.startsWith("/")) {
    let handled = await interceptSlashCommand(appState.query);
    if (handled) {
      appState.query = "";
      return;
    }
  }

  if (appState.isThinking) {
    appState.pendingUserQueries.push(appState.query);
    appState.query = "";
    addLog(t("sys.queue_wait"));
    return;
  }

  let userQuery = appState.query;
  
  let textAttachments = appState.attachedFiles.filter(f => f.type === "document");
  if (textAttachments.length > 0) {
      let combined = "";
      for (let file of textAttachments) {
          if (file.data.startsWith("[첨부 파일 참조:")) {
              combined += `\n\n${file.data}`;
          } else {
              combined += `\n\n--- [Attached Document: ${file.name}] ---\n${file.data}\n---`;
          }
      }
      userQuery = `${userQuery}${combined}`;
  }
  
  let imageAttachments = appState.attachedFiles.filter(f => f.type === "image").map(f => f.data);

  appState.messages = [...appState.messages, { role: "user", content: userQuery, logs: [], images_base64: imageAttachments }];
  appState.messages = [...appState.messages, { role: "assistant", content: "", logs: [] }];
  appState.query = "";
  appState.attachedFiles = [];
  appState.isThinking = true;
  appState.elapsedSec = 0;
  
  if (appState.timerInterval !== null) clearInterval(appState.timerInterval);
  appState.timerInterval = setInterval(() => {
    appState.elapsedSec += 1;
  }, 1000);

  await internalExecuteAgent();
}
