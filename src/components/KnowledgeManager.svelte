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

  let scheduleData: any = $state(null);
  
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
                  scheduleData = JSON.parse(content);
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

  let nextExecutionText = $derived.by(() => {
    if (!scheduleData) return t("knowledge.sched_not_set");
    if (scheduleData.cron_expression && scheduleData.cron_expression.trim() !== "") {
        return t("knowledge.sched_cron_set", { cron: scheduleData.cron_expression });
    }
    if (!scheduleData.interval_seconds) return t("knowledge.sched_not_set");
    let baseTime = scheduleData.last_run ? new Date(scheduleData.last_run) : new Date();
    let nextTime = new Date(baseTime.getTime() + scheduleData.interval_seconds * 1000);
    return t("knowledge.sched_next", { time: nextTime.toLocaleString(appState.config.language === "ko" ? "ko-KR" : "en-US", { dateStyle: "long", timeStyle: "medium" }) });
  });
</script>

<div style="flex: 1; display: flex; flex-direction: row; overflow: hidden;">
  {#if appState.showSysSidebar}
    <div class="sys-sidebar">
      {#if appState.sysModalDomain !== "logs"}
        <div style="display: flex; gap: 8px; margin-bottom: 8px; padding: 0 16px;">
          <button class="sys-create-btn" style="flex: 1; margin: 0; width: auto;" onclick={createSysItem}>{t("knowledge.create")}</button>
          {#if appState.sysModalItems.length > 0}
            <button class="sys-create-btn sys-danger" style="flex: none; margin: 0; width: auto; padding: 0 12px; font-weight: bold; font-size: 1.1em" onclick={deleteAllSysItems} title={t("knowledge.deleteAll")}>🗑</button>
          {/if}
        </div>
      {:else}
        <div style="display: flex; gap: 8px; margin-bottom: 8px; padding: 0 16px;">
        {#if appState.selectedLogs.length > 0}
          <button class="sys-create-btn sys-danger" style="flex: 1; margin: 0; width: auto; font-weight: bold;" onclick={deleteSelectedLogs}>{t("knowledge.deleteSelected", { count: appState.selectedLogs.length })}</button>
        {/if}
        {#if appState.sysModalItems.length > 0}
          <button class="sys-create-btn sys-danger" style="flex: none; margin: 0; width: auto; padding: 0 12px; font-weight: bold; font-size: 1.1em" onclick={deleteAllSysItems} title={t("knowledge.deleteAll")}>🗑</button>
        {/if}
        </div>
      {/if}
      {#if appState.sysModalItems.length === 0}
        <div style="padding:10px; color:#a1a1aa; font-size:0.85rem">{t("knowledge.empty")}</div>
      {/if}
      {#each appState.sysModalItems as item}
        <div class="sys-item">
          {#if appState.sysModalDomain === "logs"}
            <input
              type="checkbox"
              style="margin-right: 8px;"
              checked={appState.selectedLogs.includes(item.name)}
              onchange={(e) => {
                let target = e.target as HTMLInputElement;
                if (target.checked) appState.selectedLogs = [...appState.selectedLogs, item.name];
                else appState.selectedLogs = appState.selectedLogs.filter((n: string) => n !== item.name);
              }}
            />
          {/if}
          <button class="sys-link" onclick={() => loadSysItem(item.name)}>{item.name}</button>
          {#if appState.sysModalDomain !== "logs"}
            <button class="sys-del" onclick={() => deleteSysItem(item.name)}>🗑</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  <div class="sys-content">
    {#if appState.viewingItemContent === null}
      <div class="sys-placeholder">{t("knowledge.placeholder")}</div>
    {:else}
      <div class="sys-content-header">
        <div style="display:flex; align-items:center; gap: 12px;">
          <button class="sys-save-btn" onclick={() => (appState.showSysSidebar = !appState.showSysSidebar)} style="background:transparent; color:#1a1a1a; border:1px solid #1a1a1a; padding: 4px 8px; text-transform:none;">{appState.showSysSidebar ? t("knowledge.hideList") : t("knowledge.showList")}</button>
          <div style="font-weight:600;">{appState.viewingItemName}</div>
        </div>
        {#if ["skills", "rules", "workflows", "brain", "schedules"].includes(appState.sysModalDomain)}
          <button class="sys-save-btn" onclick={saveSysItem}>{t("knowledge.save")}</button>
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
              <label for="schedInterval">{t("knowledge.label_interval")}</label>
              <input id="schedInterval" type="number" bind:value={scheduleData.interval_seconds} oninput={syncScheduleData} placeholder={t("knowledge.ph_interval")} />
            </div>
            <div class="form-group" style="flex: 1;">
              <label for="schedCron">{t("knowledge.label_cron")}</label>
              <input id="schedCron" type="text" bind:value={scheduleData.cron_expression} oninput={syncScheduleData} placeholder={t("knowledge.ph_cron")} />
            </div>
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
        <textarea class="sys-textarea" bind:value={appState.viewingItemContent} placeholder={t("knowledge.ph_edit_content")} readonly={!["skills", "rules", "workflows", "brain", "schedules"].includes(appState.sysModalDomain)}></textarea>
      {/if}
    {/if}
  </div>
</div>

<style>
  .sys-sidebar {
    width: 250px;
    min-width: 250px;
    flex-shrink: 0;
    background: #f0ede1;
    border-right: 1px solid #1a1a1a;
    overflow-y: auto;
    padding: 12px 0;
  }

  .sys-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    border-bottom: 1px solid #ebebeb;
    flex-shrink: 0;
  }
  .sys-item:hover {
    background: #ebe8de;
  }
  .sys-link {
    background: none;
    border: none;
    color: #1a1a1a;
    font-size: 0.95rem;
    text-align: left;
    cursor: pointer;
    flex: 1;
    min-width: 0;
    padding: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sys-del {
    background: none;
    border: none;
    color: #e0005a;
    font-size: 1.2rem;
    cursor: pointer;
    opacity: 0.85;
    padding: 6px;
    transition: all 0.2s ease;
  }
  .sys-del:hover {
    opacity: 1;
    transform: scale(1.2);
  }
  .sys-create-btn {
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    color: #1a1a1a;
    padding: 12px;
    border-radius: 0;
    width: calc(100% - 32px);
    margin: 0 16px;
    cursor: pointer;
    font-size: 0.9rem;
    font-weight: 500;
  }
  .sys-create-btn:hover {
    background: #1a1a1a;
    color: #fcfbf8;
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
  .sys-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #a1a1aa;
    font-style: italic;
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
