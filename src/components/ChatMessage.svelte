<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  
  DOMPurify.addHook('afterSanitizeAttributes', function(node) {
    if ('target' in node) {
      node.setAttribute('target', '_blank');
      node.setAttribute('rel', 'noopener noreferrer');
    }
  });

  export let msg: { role: string; content: string; logs?: string[] };
  export let isThinking: boolean = false;
  export let isLast: boolean = false;
  export let onViewLogs: (logs: string[]) => void = () => {};

  function viewLogs() {
    if (msg.logs) {
      onViewLogs(msg.logs);
    }
  }

  function formatContent(content: string): string {
    // 1. Match standard and deepseek tags: <think>...</think>, <thought>...</thought>, <|think|>...</|think|>
    let processed = content.replace(/<(?:\|)?(?:think|thought)(?:\|)?>([\s\S]*?)<\/(?:\|)?(?:think|thought)(?:\|)?>/gi, (match, p1) => {
        return `<details class="reasoning-block"><summary class="reasoning-summary">🧠 AI 사고 과정 (Thinking Process)</summary><div class="reasoning-content">${p1.trim()}</div></details>\n\n`;
    });
    // 2. Match Gemma 4 specific raw chat template tokens: <|channel>thought ... <channel|>
    processed = processed.replace(/<\|channel>thought([\s\S]*?)<channel\|>/gi, (match, p1) => {
        return `<details class="reasoning-block"><summary class="reasoning-summary">🧠 AI 사고 과정 (Thinking Process)</summary><div class="reasoning-content">${p1.trim()}</div></details>\n\n`;
    });
    return DOMPurify.sanitize(marked.parse(processed) as string, { ADD_ATTR: ['target'], ADD_TAGS: ['details', 'summary'] });
  }
</script>

<div class="message {msg.role}">
  <div class="avatar">{msg.role === "assistant" ? "🤖" : "🧑‍💻"}</div>
  <div class="bubble" style="display:flex; flex-direction:column; gap:8px;">
    {#if isThinking && msg.role === "assistant" && isLast}
      <span class="thinking-text" style="color: #6b7280; font-size: 0.9rem; font-style: italic;">
        {msg.logs && msg.logs.length > 0 ? msg.logs[msg.logs.length - 1] : "Thinking..."}
      </span>
    {:else}
      <div>
        {@html formatContent(msg.content)}
      </div>
      {#if msg.logs && msg.logs.length > 0}
        <div style="margin-top: 8px; border-top: 1px dashed #d1d5db; padding-top: 8px;">
          <button on:click={viewLogs} style="background:transparent; border:1px solid #9ca3af; border-radius:4px; font-size:0.75rem; cursor:pointer; color:#4b5563;">
            📄 View Steps ({msg.logs.length})
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .message {
    display: flex;
    gap: 24px;
    align-items: flex-start;
    max-width: 85%;
    animation: fadeIn 0.3s ease-out;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .message.user {
    align-self: flex-end;
    flex-direction: row-reverse;
  }

  .avatar {
    font-size: 1.1rem;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 0;
    flex-shrink: 0;
    color: #1a1a1a;
  }

  .bubble {
    background: transparent;
    padding: 0;
    font-size: 1.05rem;
    line-height: 1.7;
    color: #1a1a1a;
  }
  .message.user .bubble {
    background: #ebe8de;
    border: 1px solid #1a1a1a;
    color: #1a1a1a;
    border-radius: 0;
    padding: 8px 16px;
    line-height: 1.4;
  }
  .message.assistant .bubble {
    width: 100%;
  }
  
  /* Remove default paragraph margins inside all bubbles to prevent unwanted vertical stretching */
  .bubble :global(p) { margin: 0; }
  .bubble :global(p:not(:last-child)) { margin-bottom: 12px; }

  /* Reasoning Block Styling (Global because of @html insertion) */
  :global(.reasoning-block) {
    background: #ebe8de;
    padding: 10px 14px;
    border-radius: 8px;
    margin-bottom: 14px;
    border: 1px solid #d1d5db;
    font-size: 0.95rem;
    box-shadow: inset 0 1px 3px rgba(0,0,0,0.03);
  }
  :global(.reasoning-summary) {
    cursor: pointer;
    font-weight: 600;
    color: #4b5563;
    outline: none;
    user-select: none;
    transition: color 0.2s;
  }
  :global(.reasoning-summary:hover) {
    color: #1a1a1a;
  }
  :global(.reasoning-content) {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px dashed #d1d5db;
    white-space: pre-wrap;
    color: #4b5563;
    font-family: inherit;
    line-height: 1.5;
  }
</style>
