<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  DOMPurify.addHook('afterSanitizeAttributes', function(node) {
    if ('target' in node) {
      node.setAttribute('target', '_blank');
      node.setAttribute('rel', 'noopener noreferrer');
    }
  });
  import { tick } from "svelte";
  import {
    appState,
    interceptSlashCommand,
    submitQuery,
    stopAgent,
    wipeChat,
    compressChatMemory,
    deleteSysItem,
    showError,
  } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import ChatMessage from "./ChatMessage.svelte";
  let fileInput: HTMLInputElement;

  async function handleFileSelect(e: Event) {
    const target = e.target as HTMLInputElement;
    if (!target.files) return;
    
    for (const file of Array.from(target.files)) {
      const isImage = file.type.startsWith('image/');
      const isVideo = file.type.startsWith('video/');
      const isText = file.type.startsWith('text/') || file.name.endsWith('.txt') || file.name.endsWith('.json') || file.name.endsWith('.md') || file.name.endsWith('.csv');

      let result = "";
      
      if (isVideo || (!isImage && !isText)) {
         result = t("chat.attach_ref", { name: file.name });
         appState.attachedFiles = [
            ...(appState.attachedFiles || []), 
            { type: 'document', name: file.name, data: result, file }
         ];
      } else {
         result = await new Promise<string>((resolve) => {
            const reader = new FileReader();
            reader.onload = (ev) => {
               let res = ev.target?.result as string;
               if (isImage && res.includes(',')) res = res.split(',')[1];
               resolve(res);
            };
            if (isImage) {
               reader.readAsDataURL(file);
            } else {
               reader.readAsText(file);
            }
         });
         
         appState.attachedFiles = [
            ...(appState.attachedFiles || []), 
            { type: isImage ? 'image' : 'document', name: file.name, data: result, file }
         ];
      }
    }
    target.value = '';
  }

  function removeFile(index: number) {
     appState.attachedFiles = appState.attachedFiles.filter((_, i) => i !== index);
  }

  let textareaElement: HTMLTextAreaElement | null = $state(null);
  $effect(() => {
    if (textareaElement && typeof appState.query === "string") {
      tick().then(() => {
        if (textareaElement) {
          textareaElement.style.height = '25px';
          let newHeight = Math.min(Math.max(textareaElement.scrollHeight, 25), 200);
          textareaElement.style.height = newHeight + 'px';
          textareaElement.style.overflow = textareaElement.scrollHeight > 200 ? 'auto' : 'hidden';
        }
      });
    }
  });
  $effect(() => {
    if (appState.messages) {
      scrollToBottom();
    }
  });

  async function scrollToBottom() {
    await tick();
    if (appState.chatBoxElement) {
      appState.chatBoxElement.scrollTop = appState.chatBoxElement.scrollHeight;
    }
  }

  // removed isVoiceMode
  let isRecording = $state(false);
  let recognitionRef: any = null;

  function startVoiceInput() {
    const SpeechRecognition = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
    if (!SpeechRecognition) {
      showError(t("chat.voice_not_supported"));
      return;
    }
    
    // Clear previous query when starting fresh PTT
    appState.query = "";
    
    const recognition = new SpeechRecognition();
    recognitionRef = recognition;
    recognition.lang = appState.config.language === "ko" ? 'ko-KR' : 'en-US';
    recognition.interimResults = true;
    recognition.maxAlternatives = 1;

    recognition.onstart = () => {
      isRecording = true;
    };

    recognition.onresult = (event: any) => {
      let finalTranscript = '';
      let interimTranscript = '';
      for (let i = event.resultIndex; i < event.results.length; ++i) {
        if (event.results[i].isFinal) {
          finalTranscript += event.results[i][0].transcript;
        } else {
          interimTranscript += event.results[i][0].transcript;
        }
      }
      appState.query = finalTranscript + interimTranscript;
    };

    recognition.onerror = (event: any) => {
      console.error("Speech recognition error", event.error);
      showError(t("chat.voice_error", { err: event.error }));
      isRecording = false;
      recognitionRef = null;
    };

    recognition.onend = () => {
      isRecording = false;
      recognitionRef = null;
    };

    recognition.start();
  }

  function stopVoiceInput() {
    if (recognitionRef && isRecording) {
      recognitionRef.stop();
    }
  }

  let viewingLogs = $state<string[] | null>(null);

  async function handleSelectSuggestion(text: string) {
    if (appState.isThinking) return;
    appState.query = text;
    await tick();
    submitQuery();
  }
</script>

{#if viewingLogs}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={() => (viewingLogs = null)}>
    <div class="modal-content" onclick={(e) => e.stopPropagation()}>
      <div style="display:flex; justify-content:space-between; align-items:center; border-bottom:1px solid #e5e7eb; padding-bottom:8px; margin-bottom:12px;">
        <h3 style="margin:0;">📄 Step Logs</h3>
        <button onclick={() => (viewingLogs = null)} style="background:transparent; border:none; font-size:1.2rem; cursor:pointer;">✖</button>
      </div>
      <div style="flex:1; overflow-y:auto; font-family:monospace; font-size:0.85rem; color:#1a1a1a; white-space:pre-wrap; background:#f9fafb; padding:12px;">
        {viewingLogs.join("\n")}
      </div>
    </div>
  </div>
{/if}

<div class="chat-section">
  <div class="chat-header">
    <h1 style="display:flex; align-items:center; gap:8px;">
      PumAgent
      <button
        class="icon-btn"
        onclick={() => interceptSlashCommand("/settings")}
        aria-label="Settings"
        title={t("chat.settings_hover")}
        style="background:transparent; border:none; cursor:pointer; font-size:1.2rem; padding:4px;"
      >
        ⚙️
      </button>
    </h1>
    <div class="header-buttons">
      {#if appState.messages.length > 4}
        <button
          class="toggle-btn"
          onclick={() => compressChatMemory()}
          aria-label="Compress Memory"
          title={t("chat.compress_memory")}
          style="font-size: 1.2rem; background: transparent; border: none; padding: 4px; cursor: pointer;"
        >
          🗜
        </button>
      {/if}
      <button
        class="toggle-btn"
        onclick={() => (appState.logExpanded = !appState.logExpanded)}
        aria-label="Toggle Logs"
        style="font-size: 1.2rem; background: transparent; border: none; padding: 4px; cursor: pointer;"
      >
        {appState.logExpanded ? "▶" : "◀"}
      </button>
    </div>
  </div>

  <div class="chat-box" bind:this={appState.chatBoxElement}>
    {#each appState.messages as msg}
      <ChatMessage 
        {msg} 
        isThinking={appState.isThinking} 
        isLast={msg === appState.messages[appState.messages.length - 1]} 
        onViewLogs={(logs) => { viewingLogs = logs; }} 
        onSelectSuggestion={handleSelectSuggestion}
      />
    {/each}
  </div>

  <div class="input-area">
    <div class="attachment-box">
      <div class="attachment-action-bar">
        <div>
          <input type="file" bind:this={fileInput} multiple hidden onchange={handleFileSelect} />
          <button class="attach-chip-btn" onclick={() => fileInput.click()} aria-label="Attach File" title={t("chat.attachment")}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right:6px;"><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"></path></svg>
            {t("chat.attachment")}
          </button>
        </div>
        <div>
          {#if appState.messages.length > 1}
            <button class="clear-chat-thin-btn" onclick={wipeChat} aria-label="Wipe Chat" title={t("chat.wipe_title")}>
              Clear
            </button>
          {/if}
        </div>
      </div>

      <!-- Attachment Preview Area -->
      {#if appState.attachedFiles.length > 0}
        <div class="attachment-preview-container">
          {#if appState.isAttachListMinimized}
            <button class="minimize-btn" onclick={() => appState.isAttachListMinimized = false}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right:6px;"><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"></path></svg>
              {t("chat.attach_count_badge", { count: appState.attachedFiles.length })}
            </button>
          {:else}
            <div class="attachment-header">
              <span class="attachment-label">{t("chat.attach_waiting", { count: appState.attachedFiles.length })}</span>
              <button class="icon-btn-small" onclick={() => appState.isAttachListMinimized = true} title={t("chat.attach_min")}>
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
              </button>
            </div>
            <div class="attachment-list">
              {#each appState.attachedFiles as file, index}
                <div class="attachment-item">
                  {#if file.type === 'image'}
                    <svg class="file-icon-svg" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>
                  {:else}
                    <svg class="file-icon-svg" viewBox="0 0 24 24"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>
                  {/if}
                  <div class="file-name" title={file.name}>{file.name}</div>
                  <button class="remove-btn" aria-label="Remove file" onclick={() => removeFile(index)}>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="input-wrapper">
        <textarea
          bind:this={textareaElement}
          class="main-input"
          placeholder={appState.isThinking ? "Agent is processing..." : t("chat.placeholder")}
          bind:value={appState.query}
          disabled={appState.isThinking}
          rows="1"
          style="resize: none; overflow: hidden; max-height: 200px; opacity: {appState.isThinking ? 0.5 : 1};"
          onkeydown={(e) => {
            if (e.key === "Tab" && e.shiftKey) {
              e.preventDefault();
              appState.config.useMultiAgentWorkflow = !appState.config.useMultiAgentWorkflow;
              return;
            }
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submitQuery();
            }
          }}
        ></textarea>
      
      <div class="input-actions" style="align-self: flex-end; margin-bottom: 4px;">
        {#if appState.isThinking}
          <button
            class="icon-btn stop-btn"
            onclick={stopAgent}
            aria-label="Stop Agent"
            title={t("chat.stop")}>⛔</button
          >
        {:else}
          <button class="icon-btn toggle-voice-btn" class:recording={isRecording} onclick={() => { if(isRecording) stopVoiceInput(); else startVoiceInput(); }} title={t("chat.voice_title")}>
            {isRecording ? '🛑' : '🎙️'}
          </button>
          <button class="icon-btn send-btn" onclick={submitQuery} title={t("chat.send")} style="color:#1a1a1a;">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10" fill="#1a1a1a" stroke="#1a1a1a"></circle>
              <path d="M12 16V8" stroke="#f5f3ed"></path>
              <path d="M8 12l4-4 4 4" stroke="#f5f3ed"></path>
            </svg>
          </button>
        {/if}
      </div>
    </div>

    <div class="status-bar" style="display:flex; justify-content:space-between; margin-top:8px; font-size:0.8rem; color:#6b7280; font-weight:600;">
      <div style="display:flex; gap:16px;">
        <div>
          <span>{t("chat.mode")}: <span style="color:#1a1a1a;">{appState.config.useMultiAgentWorkflow ? t("chat.multi_accuracy") : t("chat.single_fast")}</span></span>
          <span style="margin-left:6px; opacity:0.7;">(Shift+Tab)</span>
        </div>
        <div>Max Loops: {appState.config.maxLoops}</div>
      </div>
      <div>
        {#if appState.config.heartbeatEnabled}
          <div style="display:flex; align-items:center; gap:4px; font-weight:600; color:#e0005a;" title="Heartbeat Remaining">
            <svg class="heart-icon {appState.heartbeatRemainingSec <= 5 ? 'fast-beat' : 'normal-beat'}" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none">
              <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"></path>
            </svg>
            {#if appState.isThinking && appState.heartbeatRemainingSec === 0}
              <span>Waiting...</span>
            {:else}
              <span style="font-variant-numeric: tabular-nums;">{appState.heartbeatRemainingSec}s</span>
            {/if}
          </div>
        {/if}
        {#if appState.isThinking}
          <span style="color:#e0005a; animation:pulse 1s infinite alternate;">Elapsed: {appState.elapsedSec}s</span>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  /* Zero Border Radius, High Contrast Line */
  .chat-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: #f5f3ed;
    border-right: 1px solid #1a1a1a;
    z-index: 10;
    position: relative;
    min-width: 0;
  }

  .header-buttons {
    display: flex;
    gap: 8px;
  }

  .chat-header {
    padding: 16px 28px;
    background: #f5f3ed;
    border-bottom: 1px solid #1a1a1a;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .chat-header h1 {
    margin: 0;
    font-size: 1.4rem;
    font-weight: 600;
    color: #1a1a1a;
  }

  .toggle-btn {
    background: transparent;
    border: 1px solid #1a1a1a;
    padding: 6px 14px;
    border-radius: 0; /* Anti-pattern: standard heavily rounded components */
    color: #1a1a1a;
    cursor: pointer;
    font-family: "Inter", sans-serif;
    font-size: 0.85rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    transition: all 0.2s ease;
  }
  .toggle-btn:hover {
    background: #1a1a1a;
    color: #f5f3ed;
  }

  .chat-box {
    flex: 1;
    overflow-y: auto;
    padding: 32px 28px;
    display: flex;
    flex-direction: column;
    gap: 32px;
    scroll-behavior: smooth;
  }



  .input-area {
    position: relative;
    padding: 12px 16px;
    background: #f5f3ed;
    border-top: none;
    display: flex;
    flex-direction: column;
    z-index: 10;
  }
  .input-wrapper {
    display: flex;
    flex: 1;
    background: #ebe8de;
    border-radius: 24px;
    padding: 6px 12px 6px 20px;
    align-items: flex-end;
    transition: background 0.2s ease;
  }
  .input-wrapper:focus-within {
    background: #fcfbf8;
    box-shadow: inset 0 0 0 1px #1a1a1a;
  }

  .attachment-box {
    margin-bottom: 8px;
    padding: 0 4px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .attachment-action-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .attach-chip-btn {
    background: transparent;
    border: 1px solid #d4d4d8;
    color: #4b5563;
    padding: 6px 14px;
    border-radius: 20px;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    transition: all 0.2s;
  }
  .attach-chip-btn:hover {
    background: #e5e7eb;
    color: #1a1a1a;
  }
  .clear-chat-thin-btn {
    background: transparent;
    border: 1px solid #e0005a;
    color: #e0005a;
    padding: 4px 12px;
    border-radius: 16px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.2s ease;
  }
  .clear-chat-thin-btn:hover {
    background: #e0005a;
    color: #fff;
  }

  .attachment-preview-container {
    background: #fff;
    border: 1px solid #d4d4d8;
    border-radius: 16px;
    padding: 12px 16px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.03);
  }
  .minimize-btn {
    background: #f3f4f6;
    border: none;
    padding: 6px 14px;
    border-radius: 20px;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    color: #374151;
    transition: background 0.2s;
  }
  .minimize-btn:hover {
    background: #e5e7eb;
  }
  .attachment-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  .attachment-label {
    font-size: 0.75rem;
    font-weight: 700;
    color: #6b7280;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .icon-btn-small {
    background: none;
    border: none;
    cursor: pointer;
    color: #6b7280;
    display: flex;
    align-items: center;
    padding: 2px;
    transition: color 0.2s;
  }
  .icon-btn-small:hover { color: #1a1a1a; }
  
  .attachment-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .attachment-item {
    display: flex;
    align-items: center;
    background: #f3f4f6;
    border: 1px solid transparent;
    padding: 6px 10px 6px 12px;
    border-radius: 12px;
    gap: 8px;
    font-size: 0.8rem;
    color: #1f2937;
    max-width: 220px;
    transition: all 0.2s;
  }
  .attachment-item:hover {
    background: #e5e7eb;
    border-color: #d1d5db;
  }
  .file-icon-svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: #6b7280;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex-shrink: 0;
  }
  .file-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    font-weight: 500;
  }
  .remove-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: #d1d5db;
    border: none;
    cursor: pointer;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: #4b5563;
    padding: 0;
    flex-shrink: 0;
    transition: all 0.2s;
  }
  .remove-btn:hover {
    background: #ef4444;
    color: #fff;
  }

  .input-area .main-input {
    flex: 1;
    padding: 0;
    margin: 8px 0;
    height: 25px;
    min-height: 25px;
    border: none;
    background: transparent;
    color: #1a1a1a;
    font-size: 1.05rem;
    font-family: inherit;
    line-height: 25px;
  }
  .input-area .main-input:focus {
    outline: none;
  }



  .toggle-voice-btn.recording {
    color: #ef4444;
    animation: pulse 1s infinite alternate;
  }

  .input-actions {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .input-actions .icon-btn {
    background: transparent;
    border: none;
    font-size: 1.3rem;
    cursor: pointer;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background 0.2s ease,
      opacity 0.2s ease;
    opacity: 0.75;
  }
  .input-actions .icon-btn:hover {
    background: #d4d4d8;
    opacity: 1;
  }
  .input-actions .stop-btn {
    opacity: 1;
  }
  @keyframes pulse {
    from { color: #1a1a1a; }
    to { color: #e0005a; }
  }

  @keyframes beat {
    0% { transform: scale(1); }
    15% { transform: scale(1.3); }
    30% { transform: scale(1); }
    45% { transform: scale(1.3); }
    60% { transform: scale(1); }
    100% { transform: scale(1); }
  }

  .heart-icon.normal-beat {
    animation: beat 2s infinite cubic-bezier(0.215, 0.61, 0.355, 1);
  }
  
  .heart-icon.fast-beat {
    animation: beat 1s infinite cubic-bezier(0.215, 0.61, 0.355, 1);
  }

  .modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.5);
    z-index: 1000;
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .modal-content {
    background: #fff;
    width: 600px;
    max-width: 90vw;
    height: 80vh;
    display: flex;
    flex-direction: column;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0,0,0,0.1);
  }
</style>
