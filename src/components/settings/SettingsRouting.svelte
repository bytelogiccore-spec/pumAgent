<script lang="ts">
  import { appState, showError } from "../../lib/store.svelte";
  
  function addEndpoint() {
    appState.config.endpoints = [...appState.config.endpoints, {
      id: "ep-" + Math.random().toString(36).substr(2, 6),
      name: "New AI Provider",
      api_url: "https://",
      model: "",
      api_key: "",
      is_enabled: true
    }];
  }

  function removeEndpoint(idx: number) {
    if (appState.config.endpoints.length <= 1) {
      showError("At least one AI Provider is required.");
      return;
    }
    appState.config.endpoints = appState.config.endpoints.filter((_, i) => i !== idx);
  }

  const FIXED_URLS = [
    "https://openrouter.ai/api/v1/chat/completions",
    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    "https://api.anthropic.com/v1/messages",
    "https://api.groq.com/openai/v1/chat/completions",
    "https://api.deepseek.com/chat/completions",
    "https://api.mistral.ai/v1/chat/completions",
    "https://api.together.xyz/v1/chat/completions",
    "https://api.endpoints.anyscale.com/v1/chat/completions",
    "https://api.openai.com/v1/chat/completions"
  ];

  function applyPreset(ep: any, e: Event) {
    const preset = (e.target as HTMLSelectElement).value;
    if (preset === "custom") {
      ep.name = "Custom Provider";
      ep.api_url = "http://localhost:8000/v1/chat/completions";
    } else if (preset === "openrouter") {
      ep.name = "OpenRouter";
      ep.api_url = "https://openrouter.ai/api/v1/chat/completions";
    } else if (preset === "gemini") {
      ep.name = "Google Gemini";
      ep.api_url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";
    } else if (preset === "claude") {
      ep.name = "Anthropic Claude";
      ep.api_url = "https://api.anthropic.com/v1/messages";
    } else if (preset === "groq") {
      ep.name = "Groq";
      ep.api_url = "https://api.groq.com/openai/v1/chat/completions";
    } else if (preset === "deepseek") {
      ep.name = "DeepSeek";
      ep.api_url = "https://api.deepseek.com/chat/completions";
    } else if (preset === "mistral") {
      ep.name = "Mistral AI";
      ep.api_url = "https://api.mistral.ai/v1/chat/completions";
    } else if (preset === "together") {
      ep.name = "Together AI";
      ep.api_url = "https://api.together.xyz/v1/chat/completions";
    } else if (preset === "anyscale") {
      ep.name = "AnyScale";
      ep.api_url = "https://api.endpoints.anyscale.com/v1/chat/completions";
    } else if (preset === "openai") {
      ep.name = "OpenAI";
      ep.api_url = "https://api.openai.com/v1/chat/completions";
    } else if (preset === "local") {
      ep.name = "Local (llama.cpp)";
      ep.api_url = "http://127.0.0.1:8000/v1/chat/completions";
    }

    (e.target as HTMLSelectElement).value = ""; // reset dropdown
  }
</script>

<div style="margin-bottom: 24px;">
  <div style="display:flex; justify-content:space-between; align-items:flex-end; margin-bottom: 12px;">
    <div>
      <div style="font-size: 1rem; font-weight: 600; color: #e4e4e7; margin-bottom: 4px;">🧠 LLM Providers</div>
      <div style="font-size: 0.8rem; color: #a1a1aa;">Register and manage your AI models. Scroll horizontally to view all.</div>
    </div>
    <button class="sys-badge" onclick={addEndpoint}>+ Add Provider</button>
  </div>

  <!-- Carousel Container -->
  <div class="carousel-container">
    {#each appState.config.endpoints as ep, idx}
    <div class="carousel-card" style="border-top: 3px solid {ep.is_enabled ? '#10b981' : '#ef4444'};">
      
      <div style="display:flex; justify-content:space-between; align-items:center; border-bottom: 1px solid rgba(255,255,255,0.1); padding-bottom: 8px;">
        <label style="display: flex; align-items: center; gap: 6px; font-size: 0.85rem; font-weight: 600; color: {ep.is_enabled ? '#10b981' : '#ef4444'}; cursor: pointer;">
          <input type="checkbox" bind:checked={ep.is_enabled} /> {ep.is_enabled ? 'Active' : 'Disabled'}
        </label>
        <button class="remove-btn" onclick={() => removeEndpoint(idx)}>Delete</button>
      </div>

      <div class="form-group">
        <label>Provider Name
          {#if FIXED_URLS.includes(ep.api_url) || ep.api_url === "http://127.0.0.1:8000/v1/chat/completions"}
            <div style="font-size: 0.95rem; font-weight: 500; color: #a1a1aa; background: rgba(255,255,255,0.03); padding: 12px; border: 1px solid rgba(255, 255, 255, 0.05); border-radius: 4px; display:flex; align-items:center; gap: 8px;">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
              {ep.name}
            </div>
          {:else}
            <input type="text" bind:value={ep.name} placeholder="e.g. Local Gemma" />
          {/if}
        </label>
      </div>

      <div class="form-group" style="display:flex; gap: 8px;">
        <label style="flex:1;">Model ID
          <input type="text" bind:value={ep.model} placeholder="gemma-4" />
        </label>
      </div>

      <details class="adv-config">
        <summary>⚙️ Advanced Config (API URL & Keys)</summary>
        <div class="adv-content">
          <div class="form-group">
            <label>Template (Auto-fill URL)
              <select class="sys-select" onchange={(e) => applyPreset(ep, e)}>
                <option value="">-- Select Preset --</option>
                <option value="openrouter">OpenRouter (All Models)</option>
                <option value="gemini">Google Gemini AI</option>
                <option value="claude">Anthropic Claude</option>
                <option value="groq">Groq</option>
                <option value="deepseek">DeepSeek</option>
                <option value="mistral">Mistral AI</option>
                <option value="together">Together AI</option>
                <option value="anyscale">AnyScale</option>
                <option value="openai">OpenAI</option>
                <option value="local">Local (llama.cpp)</option>
                <option value="custom">Custom (Edit API URL)</option>
              </select>
            </label>
          </div>
          {#if !FIXED_URLS.includes(ep.api_url)}
          <div class="form-group">
            <label>API URL (Custom)
              <input type="text" bind:value={ep.api_url} placeholder="http://127.0.0.1:8000/v1/chat/completions" />
            </label>
          </div>
          {:else}
          <div style="font-size: 0.75rem; color: #10b981; padding: 4px 0; display:flex; align-items:center; gap: 4px;">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
            Fixed API URL (Automatically managed)
          </div>
          {/if}
          <div class="form-group">
            <label>API Key (Optional)
              <input type="password" bind:value={ep.api_key} placeholder="sk-..." />
            </label>
          </div>
        </div>
      </details>
    </div>
    {/each}
  </div>

  <div style="margin-top: 20px; padding: 20px; background: #f0ede1; border: 1px solid #1a1a1a;">
    <div style="font-size: 0.95rem; font-weight: 700; color: #1a1a1a; margin-bottom: 16px; display:flex; align-items:center; gap: 6px; text-transform: uppercase; letter-spacing: 0.05em;">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter"><path d="M12 19V5"/><path d="m5 12 7-7 7 7"/></svg>
      Agent Role Routing
    </div>
    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Planner (Drafting Plans)
        <select bind:value={appState.config.plannerEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Critic (Reviewing)
        <select bind:value={appState.config.criticEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Writer (Final Synthesis)
        <select bind:value={appState.config.writerEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Worker (Execution)
        <select bind:value={appState.config.workerEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Reflector (Background Memory)
        <select bind:value={appState.config.reflectorEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
      <label style="font-size: 0.85rem; display:flex; flex-direction:column; gap:6px; color:#1a1a1a; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
        Registry (Architecture)
        <select bind:value={appState.config.registryEndpointId} class="sys-select" style="padding: 10px;">
          {#each appState.config.endpoints as ep}
            <option value={ep.id}>{ep.name} ({ep.model})</option>
          {/each}
        </select>
      </label>
    </div>
  </div>
</div>

<style>
  .form-group label {
    color: #a1a1aa;
    font-weight: 600;
    font-size: 0.85rem;
    margin-bottom: 8px;
    display: block;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .form-group input,
  .sys-select {
    width: 100%;
    box-sizing: border-box;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 12px;
    border-radius: 0;
    color: #1a1a1a;
    font-family: inherit;
    font-size: 1rem;
    resize: none;
  }
  .form-group input:focus,
  .sys-select:focus {
    outline: none;
    border-color: #e0005a;
  }
  .sys-badge {
    flex-shrink: 0;
    background: #fcfbf8;
    border: 1px solid #1a1a1a;
    padding: 6px 16px;
    border-radius: 0;
    color: #1a1a1a;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: 0.2s;
  }
  .sys-badge:hover {
    background: #e0005a;
    color: #fcfbf8;
    border-color: #e0005a;
  }
  /* Carousel */
  .carousel-container {
    display: flex;
    overflow-x: auto;
    gap: 16px;
    padding-bottom: 24px;
    scroll-snap-type: x mandatory;
    scrollbar-width: thin;
    scrollbar-color: #52525b transparent;
  }
  .carousel-container::-webkit-scrollbar {
    height: 6px;
  }
  .carousel-container::-webkit-scrollbar-thumb {
    background-color: #52525b;
    border-radius: 4px;
  }
  .carousel-card {
    scroll-snap-align: start;
    flex: 0 0 320px;
    background: rgba(20, 20, 23, 0.6);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 4px 20px rgba(0,0,0,0.25);
    display: flex;
    flex-direction: column;
    gap: 12px;
    transition: transform 0.2s, box-shadow 0.2s;
  }
  .carousel-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0,0,0,0.4);
    border-color: rgba(255,255,255,0.15);
  }
  .remove-btn {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
  }
  .remove-btn:hover {
    background: rgba(239, 68, 68, 0.2);
  }
  /* Advanced Config Details/Summary */
  .adv-config {
    margin-top: 4px;
    background: rgba(0,0,0,0.2);
    border-radius: 6px;
    overflow: hidden;
  }
  .adv-config summary {
    font-size: 0.8rem;
    color: #a1a1aa;
    padding: 8px 12px;
    cursor: pointer;
    list-style: none; /* standard */
    font-weight: 500;
  }
  .adv-config summary::-webkit-details-marker {
    display: none; /* webkit */
  }
  .adv-config summary:hover {
    color: #e4e4e7;
    background: rgba(255,255,255,0.03);
  }
  .adv-content {
    padding: 12px;
    border-top: 1px solid rgba(255,255,255,0.05);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
