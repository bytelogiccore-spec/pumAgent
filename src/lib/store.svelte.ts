import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_PLANNER,
  DEFAULT_CRITIC,
  DEFAULT_WRITER,
  DEFAULT_REFLECTOR,
  DEFAULT_HEARTBEAT,
  DEFAULT_WORKER,
  DEFAULT_REGISTRY,
} from "./constants";
import { t } from "./i18n.svelte";

export interface AttachedFile {
  type: 'image' | 'document';
  name: string;
  data: string;
  file: File;
}

export interface LlmEndpoint {
  id: string;
  name: string;
  api_url: string;
  model: string;
  api_key: string;
  is_enabled: boolean;
}

export const appState = $state({
  globalError: null as string | null,
  logExpanded: true,
  sessionId: (() => {
    let now = new Date();
    let pad = (n: number) => n.toString().padStart(2, '0');
    return `${now.getFullYear().toString().slice(2)}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  })(),
  config: {
    isFirstRun: true,
    endpoints: [] as LlmEndpoint[],
    plannerEndpointId: "local-primary",
    criticEndpointId: "cloud-secondary",
    writerEndpointId: "cloud-secondary",
    workerEndpointId: "local-primary",
    reflectorEndpointId: "local-primary",
    registryEndpointId: "local-primary",
    maxLoops: 3,
    language: "en",
    customLanguages: [] as string[],
    systemPrompt: "You are a practical AI development assistant. Always respond in Korean.",
    searchProvider: "duckduckgo",
    tavilyApiKey: "",
    googleApiKey: "",
    googleCx: "",
    useMultiAgentWorkflow: false,
    useThinkMode: false,
    plannerPrompt: DEFAULT_PLANNER,
    criticPrompt: DEFAULT_CRITIC,
    writerPrompt: DEFAULT_WRITER,
    reflectorPrompt: DEFAULT_REFLECTOR,
    heartbeatPrompt: DEFAULT_HEARTBEAT,
    workerPrompt: DEFAULT_WORKER,
    registryPrompt: DEFAULT_REGISTRY,
    heartbeatEnabled: false,
    heartbeatInterval: 3600,
    telegramEnabled: false,
    telegramBotToken: "",
    telegramChatId: "",
  },
  kbQuota: null as any,
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
      content: "",
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
  pendingUserQueries: [] as string[],
  pendingHeartbeat: false,
  heartbeatRemainingSec: 0,
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

export function showError(message: any) {
  if (message instanceof Error) {
    appState.globalError = message.message;
  } else if (typeof message === 'string') {
    appState.globalError = message;
  } else {
    appState.globalError = JSON.stringify(message, null, 2);
  }
}

export * from "./actions";
