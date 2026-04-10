import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_PLANNER,
  DEFAULT_CRITIC,
  DEFAULT_WRITER,
  DEFAULT_REFLECTOR,
  DEFAULT_HEARTBEAT,
} from "./constants";
import { t } from "./i18n.svelte";

export interface AttachedFile {
  type: 'image' | 'document';
  name: string;
  data: string;
  file: File;
}

export const appState = $state({
  logExpanded: true,
  config: {
    isFirstRun: true,
    apiUrl: "http://127.0.0.1:8000/v1/chat/completions",
    llmApiKey: "",
    model: "gemma-4",
    maxLoops: 3,
    language: "en",
    systemPrompt: "You are a practical AI development assistant. Always respond in Korean.",
    searchProvider: "duckduckgo",
    tavilyApiKey: "",
    googleApiKey: "",
    googleCx: "",
    useMultiAgentWorkflow: false,
    plannerPrompt: DEFAULT_PLANNER,
    criticPrompt: DEFAULT_CRITIC,
    writerPrompt: DEFAULT_WRITER,
    reflectorPrompt: DEFAULT_REFLECTOR,
    heartbeatPrompt: DEFAULT_HEARTBEAT,
    heartbeatEnabled: false,
    heartbeatInterval: 3600,
    telegramEnabled: false,
    telegramBotToken: "",
    telegramChatId: "",
  },
  sysModalOpen: false,
  sysModalTitle: "",
  sysModalItems: [] as { name: string; content: string | null }[],
  sysModalDomain: "",
  viewingItemContent: null as string | null,
  viewingItemName: null as string | null,
  showSysSidebar: true,
  selectedLogs: [] as string[],
  query: "",
  messages: [
    {
      role: "assistant",
      content: "안녕하세요! 🥰 저는 사용자님의 작업을 돕기 위해 준비된 전속 AI 어시스턴트입니다.\n\n궁금한 점이나 도움이 필요한 일이 있으신가요? 무엇이든 말씀해주세요!",
      logs: []
    },
  ] as { role: string; content: string; logs?: string[]; images_base64?: string[] }[],
  logs: [] as string[],
  isThinking: false,
  elapsedSec: 0,
  timerInterval: null as ReturnType<typeof setInterval> | null,
  chatBoxElement: undefined as HTMLDivElement | undefined,
  attachedFiles: [] as AttachedFile[],
  isAttachListMinimized: false,
});

export function addLog(msg: string) {
  let now = new Date();
  let timeStr = now.toTimeString().split(" ")[0];
  let formatted = `[${timeStr}] ${msg}`;
  appState.logs = [...appState.logs, formatted];
  
  if (appState.logs.length > 500) {
    appState.logs = appState.logs.slice(appState.logs.length - 500);
  }

  if (appState.isThinking && appState.messages.length > 0) {
    let lastIndex = appState.messages.length - 1;
    let lastMsg = appState.messages[lastIndex];
    if (lastMsg.role === "assistant") {
      lastMsg.logs = lastMsg.logs || [];
      lastMsg.logs.push(formatted);
      // Re-assign to trigger Svelte reactivity
      appState.messages[lastIndex] = lastMsg;
    }
  }
}

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
    return true;
  }

  if (command === "/brain") {
    appState.sysModalDomain = "brain";
    appState.sysModalTitle = t("nav.brain");
    try {
      let files: string[] = await invoke("list_brain_artifacts");
      appState.sysModalItems = files.map((f) => ({ name: f, content: null }));
    } catch (e) {
      appState.sysModalItems = [];
    }
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = true;
    appState.selectedLogs = [];
    return true;
  }

  if (command === "/logs") {
    appState.sysModalDomain = "logs";
    appState.sysModalTitle = t("nav.logs");
    try {
      let files: string[] = await invoke("list_logs");
      appState.sysModalItems = files.map((f) => ({ name: f, content: null }));
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
      appState.sysModalItems = files.map((f) => ({ name: f, content: null }));
    } catch (e) {
      appState.sysModalItems = [];
    }
    appState.sysModalOpen = true;
    appState.viewingItemContent = null;
    appState.viewingItemName = null;
    appState.showSysSidebar = true;
    return true;
  }

  return false;
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
  if (!confirm(`정말로 ${name} 파일을 삭제하시겠습니까?`)) return;
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
    addLog(`[시스템] ${name} 삭제 완료`);
  } catch (e) {
    alert(`Delete failed: ${e}`);
  }
}

export async function deleteSelectedLogs() {
  if (appState.selectedLogs.length === 0) return;
  if (!confirm(`선택한 로그 ${appState.selectedLogs.length}개를 삭제하시겠습니까?`)) return;
  try {
    await invoke("delete_logs", { names: appState.selectedLogs });
    appState.sysModalItems = appState.sysModalItems.filter(i => !appState.selectedLogs.includes(i.name));
    if (appState.viewingItemName && appState.selectedLogs.includes(appState.viewingItemName)) {
      appState.viewingItemContent = null;
      appState.viewingItemName = null;
    }
    addLog(`[시스템] 선택한 로그 ${appState.selectedLogs.length}개 삭제 완료`);
    appState.selectedLogs = [];
  } catch (e) {
    alert(`Bulk delete failed: ${e}`);
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
    addLog(`[시스템] ${appState.viewingItemName} 저장 완료`);
    alert("성공적으로 저장되었습니다.");
  } catch (e) {
    alert(`Save failed: ${e}`);
  }
}

export async function createSysItem() {
  let promptMsg = appState.sysModalDomain === "schedules" 
    ? "새 스케줄 파일 이름을 입력하세요 (예: my_schedule.json)" 
    : "새 파일의 이름을 입력하세요 (예: my_artifact.md)";
  let name = prompt(promptMsg);
  if (!name) return;
  
  if (appState.sysModalDomain === "schedules") {
    if (!name.endsWith(".json")) {
       name = name.replace(/\.md$/, "") + ".json";
    }
  }

  let initialContent = "";
  if (appState.sysModalDomain === "schedules" && name.endsWith(".json")) {
      initialContent = `{\n  "name": "새 스케줄",\n  "interval_seconds": 3600,\n  "description": "스케줄 설명",\n  "task_prompt": "수행할 작업 지시문",\n  "last_run": null\n}`;
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
    alert(`Create failed: ${e}`);
  }
}

export function wipeChat() {
  if (!confirm("현재 채팅 기록을 모두 비우시겠습니까?")) return;
  appState.messages = [
    {
      role: "assistant",
      content: t("sys.welcome"),
      logs: [],
    },
  ];
  appState.logs = [];
  addLog("[시스템] 채팅 기록 초기화 완료");
}

export async function compressChatMemory(silent: boolean = false) {
  if (appState.messages.length <= 4) {
    if (!silent) alert("압축할 과거 대화가 충분하지 않습니다 (최소 5개 이상의 메시지 필요).");
    return;
  }
  
  if (!silent && !confirm("과거 대화를 압축하시겠습니까?\n가장 최근 대화를 제외한 이전 내역이 하나의 요약문으로 합쳐져 토큰을 크게 절약합니다.")) return;

  appState.isThinking = true;
  addLog("[시스템] 과거 대화 메모리 요약 및 압축 중...");
  
  try {
    let recentCount = 2; // retain very last user + assistant
    let oldMessages = appState.messages.slice(0, appState.messages.length - recentCount);
    let recentMessages = appState.messages.slice(appState.messages.length - recentCount);

    let payload = {
      api_url: appState.config.apiUrl,
      llm_api_key: appState.config.llmApiKey,
      model: appState.config.model,
      messages: oldMessages,
    };

    let summary: string = await invoke("compress_memory", { payload });
    
    appState.messages = [
      {
        role: "assistant",
        content: `[압축된 이전 대화 컨텍스트]\n${summary}`
      },
      ...recentMessages
    ];
    
    addLog(`[시스템] 메모리 압축 완료. (기존 ${oldMessages.length}개 메시지 병합됨)`);
  } catch (err) {
    addLog(`[시스템 압축 오류] ${err}`);
    alert(`압축 오류: ${err}`);
  } finally {
    appState.isThinking = false;
  }
}

export async function stopAgent() {
  try {
    await invoke("stop_agent");
    addLog("[시스템] 사용자 중지 요청 - 현재 처리 중인 단계를 마치면 종료됩니다.");
  } catch (e) {
    alert(`Stop failed: ${e}`);
  }
}

export async function triggerHeartbeat() {
  let userQuery = appState.config.heartbeatPrompt;
  let silentMessage = {
    role: "user",
    content: `[SYSTEM HEARTBEAT TICK] ${userQuery}`,
  };

  addLog(`[시스템] 백그라운드 하트비트 스캔 중...`);
  try {
    let payload = {
      api_url: appState.config.apiUrl,
      llm_api_key: appState.config.llmApiKey,
      model: appState.config.model,
      system_prompt: appState.config.systemPrompt,
      planner_prompt: appState.config.plannerPrompt,
      critic_prompt: appState.config.criticPrompt,
      writer_prompt: appState.config.writerPrompt,
      reflector_prompt: appState.config.reflectorPrompt,
      max_loops: appState.config.maxLoops,
      use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
      language: appState.config.language,
      messages: [...appState.messages, silentMessage].filter(
        (m) => m.role === "user" || m.role === "assistant",
      ),
    };

    let results: any = await invoke("execute_agent_tools", { payload });
    let out = results.final_output ? results.final_output.trim() : "";

    let lowerOut = out.toLowerCase();
    if (
      lowerOut === "no tasks" ||
      lowerOut === "no tasks." ||
      lowerOut.includes("no tasks")
    ) {
      addLog(`[시스템 하트비트] 발견된 특이사항 또는 스케줄 없음 (Silent Mode).`);
      
      if (appState.messages.length > 4) {
        addLog(`[시스템 하트비트] 유휴 상태 감지: 메모리 자동 압축 및 세션 초기화를 진행합니다.`);
        await compressChatMemory(true);
      }
    } else {
      appState.messages = [
        ...appState.messages,
        silentMessage,
        { role: "assistant", content: out },
      ];
    }
  } catch (err) {
    addLog(`[시스템 하트비트 오류] ${err}`);
  }
}

async function internalExecuteAgent() {
  try {
    let payload = {
      api_url: appState.config.apiUrl,
      llm_api_key: appState.config.llmApiKey,
      model: appState.config.model,
      system_prompt: appState.config.systemPrompt,
      planner_prompt: appState.config.plannerPrompt,
      critic_prompt: appState.config.criticPrompt,
      writer_prompt: appState.config.writerPrompt,
      reflector_prompt: appState.config.reflectorPrompt,
      max_loops: appState.config.maxLoops,
      use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
      language: appState.config.language,
      messages: [...appState.messages]
        .filter((m) => m.role === "user" || m.role === "assistant")
        .filter((m, i, arr) => !(i === arr.length - 1 && m.role === "assistant" && !m.content)),
    };

    let results: any = await invoke("execute_agent_tools", { payload });
    let lastIndex = appState.messages.length - 1;
    let lastMsg = appState.messages[lastIndex];
    lastMsg.content = results.final_output;
    appState.messages[lastIndex] = lastMsg;
  } catch (err) {
    addLog(`[Tauri Bridge 오류] ${err}`);
    let lastIndex = appState.messages.length - 1;
    let lastMsg = appState.messages[lastIndex];
    lastMsg.content = `오류 발생: ${err}`;
    appState.messages[lastIndex] = lastMsg;
  } finally {
    appState.isThinking = false;
    if (appState.timerInterval !== null) {
      clearInterval(appState.timerInterval);
    }
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

export async function saveSettings() {
  try {
    await invoke("save_config", {
      config: {
        is_first_run: appState.config.isFirstRun,
        api_url: appState.config.apiUrl,
        llm_api_key: appState.config.llmApiKey,
        model: appState.config.model,
        max_loops: appState.config.maxLoops,
        language: appState.config.language,
        system_prompt: appState.config.systemPrompt,
        search_provider: appState.config.searchProvider,
        tavily_api_key: appState.config.tavilyApiKey,
        google_api_key: appState.config.googleApiKey,
        google_cx: appState.config.googleCx,
        use_multi_agent_workflow: appState.config.useMultiAgentWorkflow,
        planner_prompt: appState.config.plannerPrompt,
        critic_prompt: appState.config.criticPrompt,
        writer_prompt: appState.config.writerPrompt,
        reflector_prompt: appState.config.reflectorPrompt,
        heartbeat_prompt: appState.config.heartbeatPrompt,
        heartbeat_enabled: appState.config.heartbeatEnabled,
        heartbeat_interval: appState.config.heartbeatInterval,
        telegram_enabled: appState.config.telegramEnabled,
        telegram_bot_token: appState.config.telegramBotToken,
        telegram_chat_id: appState.config.telegramChatId,
      },
    });
    appState.sysModalOpen = false;
    addLog(t("sys.config_saved"));
  } catch (err) {
    addLog(`[설정 저장 오류] ${err}`);
  }
}
