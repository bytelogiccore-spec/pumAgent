<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import {
    appState,
    createSysItem,
    deleteSelectedLogs,
    deleteAllSysItems,
    loadSysItem,
    deleteSysItem,
    saveSysItem,
  } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let scheduleData: any = $state(null);
  let scheduleType: "interval" | "cron" = $state("interval");
  let nextExecutionText: string = $state("");
  
  $effect(() => {
    let domain = appState.sysModalDomain;
    let name = appState.viewingItemName;
    let content = appState.viewingItemContent;

    untrack(() => {
      if (domain === "schedules" && name?.endsWith(".json")) {
        try {
          if (content) {
              let currentStr = scheduleData ? JSON.stringify(scheduleData, null, 2) : "";
              if (content !== currentStr) {
                  const parsed = JSON.parse(content);
                  scheduleData = parsed;
                  scheduleType = parsed.cron_expression ? "cron" : "interval";
              }
          }
        } catch (e) {
          scheduleData = null; // parse error, show raw
        }
      } else {
        scheduleData = null;
      }
    });
  });

  function syncScheduleData() {
    if (scheduleData) {
      let str = JSON.stringify(scheduleData, null, 2);
      if (appState.viewingItemContent !== str) {
        appState.viewingItemContent = str;
      }
    }
  }

  function handleTypeChange(e: any) {
    scheduleType = e.target.value;
    if (scheduleType === "interval") {
        scheduleData.cron_expression = null;
        if (!scheduleData.interval_seconds) scheduleData.interval_seconds = 3600;
    } else {
        scheduleData.interval_seconds = null;
        if (!scheduleData.cron_expression) scheduleData.cron_expression = "0 0 * * * *";
    }
    syncScheduleData();
  }

  $effect(() => {
    if (scheduleData && appState.sysModalDomain === "schedules") {
        invoke("get_next_execution_time", { config: scheduleData }).then((res: any) => {
            nextExecutionText = res;
        }).catch(() => {
            nextExecutionText = "Invalid Configuration";
        });
    }
  });

  let summarizing: boolean = $state(false);

  async function handleSummarize() {
    if (!appState.viewingItemName) return;
    summarizing = true;
    try {
      let result: string;
      if (appState.sysModalDomain === "logs") {
        result = await invoke("summarize_log_file", { name: appState.viewingItemName });
      } else {
        result = await invoke("ai_summarize_item", { 
          domain: appState.sysModalDomain, 
          name: appState.viewingItemName 
        });
      }
      appState.viewingItemContent = result;
      // Also update the list item preview if possible (optional)
    } catch (e) {
      console.error("Summarization failed:", e);
      alert("Summarization failed: " + e);
    } finally {
      summarizing = false;
    }
  }

</script>

<div style="flex: 1; display: flex; flex-direction: column; overflow: hidden; background: #fcfbf8; height: 100%;">
  {#if !appState.viewingItemName}
    <!-- LIST VIEW -->
    <div style="flex: 1; display: flex; flex-direction: column; padding: 24px; overflow: hidden;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
        <div style="font-family: 'Cormorant Garamond', serif; font-size: 1.5rem; color: #1a1a1a;">
          {appState.sysModalTitle}
        </div>
        <div style="display: flex; gap: 8px;">
          {#if appState.sysModalDomain !== "logs"}
            <button class="sys-save-btn" style="background:#fcfbf8; color:#1a1a1a; border:1px solid #1a1a1a;" onclick={createSysItem}>{t("knowledge.create")}</button>
          {/if}
          {#if appState.sysModalDomain === "logs" && appState.selectedLogs.length > 0}
            <button class="sys-save-btn sys-danger" onclick={deleteSelectedLogs}>{t("knowledge.deleteSelected", { count: appState.selectedLogs.length })}</button>
          {/if}
          {#if appState.sysModalItems.length > 0}
            <button class="sys-save-btn sys-danger" onclick={deleteAllSysItems} title={t("knowledge.deleteAll")}>🗑 {t("knowledge.deleteAll")}</button>
          {/if}
        </div>
      </div>

      {#if appState.sysModalItems.length === 0}
        <div style="color: #a1a1aa; font-style: italic; display: flex; justify-content: center; align-items: center; flex: 1;">
          {t("knowledge.empty")}
        </div>
      {:else}
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px; overflow-y: auto; align-content: start; padding-right: 8px; padding-bottom: 24px;">
          {#each appState.sysModalItems as item}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="list-item-card" ondblclick={() => loadSysItem(item.name)}>
              {#if appState.sysModalDomain === "logs"}
                <input
                  type="checkbox"
                  style="margin-right: 12px; transform: scale(1.2); cursor: pointer;"
                  checked={appState.selectedLogs.includes(item.name)}
                  onchange={(e) => {
                    let target = e.target as HTMLInputElement;
                    if (target.checked) appState.selectedLogs = [...appState.selectedLogs, item.name];
                    else appState.selectedLogs = appState.selectedLogs.filter((n: string) => n !== item.name);
                  }}
                  onclick={(e) => e.stopPropagation()}
                />
              {/if}
              <div style="flex: 1; font-weight: 500; color: #1a1a1a; cursor: default; user-select: none; word-break: break-all; font-size: 1rem;">
                📄 {item.name}
              </div>
              {#if appState.sysModalDomain !== "logs"}
                <button class="sys-del" onclick={(e) => { e.stopPropagation(); deleteSysItem(item.name); }} title="Delete">🗑</button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <!-- DETAIL VIEW -->
    <div class="sys-content" style="flex: 1; display: flex; flex-direction: column; overflow: hidden; min-height: 0;">
      <div class="sys-content-header">
        <div style="display:flex; align-items:center; gap: 12px;">
          <button class="sys-save-btn" onclick={() => { appState.viewingItemName = null; appState.viewingItemContent = null; }} style="background:transparent; color:#1a1a1a; border:1px solid #1a1a1a; padding: 4px 12px; text-transform:none;">← Back</button>
          <div style="font-weight:600;">{appState.viewingItemName}</div>
        </div>
        {#if ["skills", "rules", "workflows", "brain", "logs"].includes(appState.sysModalDomain)}
          <div style="display: flex; gap: 8px;">
            <button class="sys-save-btn" style="background:#0284c7;" onclick={handleSummarize} disabled={summarizing}>
              {summarizing ? "Summarizing..." : "✨ AI 요약"}
            </button>
            {#if appState.sysModalDomain !== "logs"}
              <button class="sys-save-btn" onclick={saveSysItem}>{t("knowledge.save")}</button>
            {/if}
          </div>
        {/if}
      </div>
      {#if appState.sysModalDomain === "logs" || (appState.sysModalDomain === "brain" && (!appState.viewingItemName || appState.viewingItemName.endsWith(".md")))}
        <div class="sys-markdown-view bubble" style="flex: 1; overflow-y: auto; padding: 24px; color: #1a1a1a;">
          {@html DOMPurify.sanitize(marked.parse(appState.viewingItemContent || "") as string, { ADD_ATTR: ['target'] })}
        </div>
      {:else if scheduleData}
        <div class="schedule-form" style="flex: 1; overflow-y: auto; padding: 24px; display: flex; flex-direction: column; gap: 20px;">
          <div class="form-group">
            <label for="schedName">{t("knowledge.label_name")}</label>
            <input id="schedName" type="text" bind:value={scheduleData.name} oninput={syncScheduleData} placeholder={t("knowledge.ph_name")} />
          </div>
          <div style="display: flex; gap: 16px;">
            <div class="form-group" style="flex: 1;">
              <label for="schedType">{t("knowledge.label_sched_type") || "Schedule Type"}</label>
              <select id="schedType" value={scheduleType} onchange={handleTypeChange} style="padding: 10px; border-radius:4px; border:1px solid #ccc; background:#fff;">
                <option value="interval">Interval (Seconds)</option>
                <option value="cron">Cron Expression</option>
              </select>
            </div>
            {#if scheduleType === "interval"}
              <div class="form-group" style="flex: 1;">
                <label for="schedInterval">{t("knowledge.label_interval")}</label>
                <input id="schedInterval" type="number" bind:value={scheduleData.interval_seconds} oninput={syncScheduleData} placeholder={t("knowledge.ph_interval")} />
              </div>
            {:else}
              <div class="form-group" style="flex: 1;">
                <label for="schedCron">{t("knowledge.label_cron")}</label>
                <input id="schedCron" type="text" bind:value={scheduleData.cron_expression} oninput={syncScheduleData} placeholder={t("knowledge.ph_cron")} />
              </div>
            {/if}
            <div class="form-group" style="flex: 1;">
              <label for="schedEndDate">{t("knowledge.label_end")}</label>
              <input id="schedEndDate" type="text" bind:value={scheduleData.end_date} oninput={syncScheduleData} placeholder={t("knowledge.ph_end")} />
            </div>
          </div>
          <div style="display: flex; gap: 16px;">
            <div class="form-group" style="flex: 1;">
              <span style="display:block; margin-bottom:8px; font-weight:600; font-size:0.85rem; color:#a1a1aa; text-transform:uppercase;">{t("knowledge.label_next_time")}</span>
              <div style="padding: 8px 12px; background: #e0f2fe; color: #0369a1; border-radius: 6px; font-weight: 600; font-size: 0.85rem; border: 1px solid #bae6fd;">
                🕒 {nextExecutionText}
              </div>
            </div>
          </div>
          <div class="form-group">
            <label for="schedDesc">{t("knowledge.label_desc")}</label>
            <input id="schedDesc" type="text" bind:value={scheduleData.description} oninput={syncScheduleData} placeholder={t("knowledge.ph_desc")} />
          </div>
          <div class="form-group" style="flex: 1; display: flex; flex-direction: column;">
            <label for="schedTask">{t("knowledge.label_task")}</label>
            <textarea id="schedTask" bind:value={scheduleData.task_prompt} oninput={syncScheduleData} placeholder={t("knowledge.ph_task")} style="flex: 1; min-height: 150px; font-family: 'Inter', sans-serif; resize: none; padding: 12px; border: 1px solid #ccc; background: #fff;"></textarea>
          </div>
          <div class="form-group">
            <label for="schedView">{t("knowledge.ph_raw_json")}</label>
            <textarea id="schedView" readonly bind:value={appState.viewingItemContent} style="height: 100px; background: #f4f4f4; border: 1px solid #ddd; padding: 8px; font-family: monospace; font-size: 0.85rem; color: #555; resize: none;"></textarea>
          </div>
        </div>
      {:else}
        <textarea class="sys-textarea" bind:value={appState.viewingItemContent} placeholder={appState.sysModalDomain === "vault" ? "Enter the new secret password here (will not be retrievable after saving)" : t("knowledge.ph_edit_content")} readonly={!["skills", "rules", "workflows", "brain", "schedules", "vault"].includes(appState.sysModalDomain)}></textarea>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sys-del {
    background: none;
    border: none;
    color: #e0005a;
    font-size: 1.2rem;
    cursor: pointer;
    opacity: 0.5;
    padding: 6px;
    transition: all 0.2s ease;
  }
  .sys-del:hover {
    opacity: 1;
    transform: scale(1.2);
  }
  .list-item-card {
    display: flex;
    align-items: center;
    background: #fff;
    border: 1px solid #e5e7eb;
    padding: 16px 20px;
    border-radius: 8px;
    transition: all 0.2s ease;
    box-shadow: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
  }
  .list-item-card:hover {
    border-color: #1a1a1a;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  }
  .list-item-card:active {
    transform: scale(0.98);
  }

  .sys-danger {
    background: transparent;
    border: 1px solid #e0005a;
    color: #e0005a;
  }
  .sys-danger:hover {
    background: #e0005a;
    color: #fff;
  }
  .sys-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 0;
    background: #fcfbf8;
    overflow: hidden;
    min-height: 0;
  }
  .sys-content-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    border-bottom: 1px solid #1a1a1a;
    color: #1a1a1a;
    font-family: "Cormorant Garamond", serif;
    font-size: 1.2rem;
    background: #f0ede1;
  }
  .sys-save-btn {
    background: #1a1a1a;
    color: #fcfbf8;
    border: none;
    padding: 8px 16px;
    border-radius: 0;
    font-family: "Inter", sans-serif;
    font-weight: 500;
    cursor: pointer;
    font-size: 0.85rem;
    text-transform: uppercase;
  }
  .sys-save-btn:hover {
    background: #e0005a;
  }

  .sys-textarea {
    flex: 1;
    width: 100%;
    resize: none;
    background: #fcfbf8;
    color: #1a1a1a;
    border: none;
    padding: 24px;
    font-family: ui-monospace, monospace;
    font-size: 0.95rem;
    line-height: 1.6;
    outline: none;
    box-sizing: border-box;
  }
  .form-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .form-group label {
    font-weight: 600;
    font-size: 0.9rem;
    color: #1a1a1a;
  }
  .form-group input {
    padding: 10px 12px;
    border: 1px solid #ccc;
    background: #fff;
    border-radius: 4px;
    font-size: 0.95rem;
    font-family: "Inter", sans-serif;
  }
  .form-group input:focus, .form-group textarea:focus {
    outline: none;
    border-color: #1a1a1a;
  }
</style>
