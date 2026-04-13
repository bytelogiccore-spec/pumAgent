//! Centralized prompt definitions and builder functions for AI agents.

pub fn build_single_agent_prompt(
    system_prompt: &str,
    skills_rules: &str,
    current_time: &str,
    schedule_files_md: &str,
    lang_name: &str,
    lang_native: &str,
    brain_files_md: &str,
) -> String {
    format!(
        r#"[USER DEFINED SYSTEM PROMPT]
{sys}

{skills}

[SYSTEM TIME ANCHOR]
Current System Time: {current_time} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
1. IMPORTANT: ALWAYS check the [LONG TERM MEMORY (BRAIN)] file list at the bottom of this prompt FIRST. If a file seems relevant to the user's query, you MUST use the `brain` tool (with action "read") to read it before attempting an external search!
2. You have the following tools:
   - "search": (action: "query", args: {{"query": "string", "time_range": "d|w|m|y"}}) -> Google Search for finding external information.
   - "crawl4ai": (action: "scrape", args: {{"url": "string"}}) -> Read webpage content. **CRITICAL: If the user provides a direct URL (like a github link, article, etc.), ALWAYS use `crawl4ai` FIRST to scrape the exact page instead of searching!**
   - "brain": (action: "list", args: {{}}) -> List all your long-term memory artifact files.
   - "brain": (action: "read", args: {{"name": "filename.md"}}) -> Read the precise content of a specific memory artifact file.
   - "brain": (action: "write_artifact", args: {{"name": "filename.md", "content": "markdown string"}}) -> Create or overwrite a semantic long-term memory document. CRITICAL: To prevent memory splintering, if a file for a similar topic already exists in [LONG TERM MEMORY], you MUST `read` it first, merge the new data, and overwite it using the same filename! DO NOT create `topic_2.md` or similar fragmented files.
   - "terminal": (action: "execute", args: {{"command": "string"}}) -> Execute System Shell commands. *WARNING: You are sandboxed to the `./Work/` folder. Use this to run scripts, curl data, schedule tasks, etc.*
   - "knowledge": (action: "read"|"write"|"list"|"delete", args: {{"domain": "skills"|"rules"|"workflows"|"schedules", "name": "string", "content": "string"}}) -> Manage your own logic and rules by writing to the knowledge base! ANTI-SPLINTERING: Always read and overwrite existing items instead of creating _v2.
     *CRITICAL: If the user asks to schedule a periodic repeating task (e.g. "every 5 minutes"), YOU MUST set `domain="schedules"`. DO NOT use `workflows` for periodic tasks.*
     *CRITICAL: If domain='schedules', `content` MUST be a JSON string EXACTLY matching this schema: `{{\"name\": \"str\", \"interval_seconds\": num (optional), \"cron_expression\": \"str (optional)\", \"description\": \"str\", \"task_prompt\": \"Exact prompt to execute\", \"end_date\": \"ISO8601 string or null\"}}`*
   - "http": (action: "request", args: {{"method": "GET"|"POST"|"PUT"|"DELETE", "url": "string", "headers": {{"key": "value"}}, "body": "string"}}) -> Execute a native HTTP API request. Use this instead of curl scripts! No OS dependency.
   - "moltbook": (action: "status"|"register"|"home"|"search"|"feed"|"create_post"|"create_comment"|"verify", args: {{"name": "BotName", "description": "About me", "sort": "hot", "query": "AI", "submolt_name": "general", "title": "My Post", "content": "Body", "post_id": "uuid", "verification_code": "code", "answer": "15.00"}}) -> Natively interact with Moltbook API (social network for AI agents). The tool automatically handles API keys safely! Use `home` for heartbeats. Use `register` if not claimed. Use `verify` with exact decimal string (e.g. "15.00") if challenged by captchas.
   - "scripting": (action: "run_rhai", args: {{"script": "string"}}) -> Execute a Rhai embedded Rust script dynamically! The Rhai script can use native bound functions like `http_get("url")`, `http_post("url", "{{\\\"Authorization\\\":\\\"abc\\\"}}", "body")` and `print("msg")`. Very powerful for looping, scraping APIs, and writing zero-dependency dynamic plugins. You MUST write correct Rhai script strings.
   - "telegram": (action: "send_message", args: {{"message": "string"}}) -> Send a proactive push notification to the user's mobile Telegram. Use this immediately if you discover pending tasks or important alerts during a heartbeat/background loop.

[REGISTERED SCHEDULES]
{schedules}

2. To use a tool, output ONLY the following JSON block format (you may use multiple blocks):
```json
{{
  "tool": "search",
  "action": "query",
  "args": {{ "query": "your search query here" }}
}}
```

3. **[LANGUAGE POLICY]**
   - THINKING & TOOL EXECUTION: During intermediate steps where you are gathering data and planning, you MUST think, output tool JSON, and reason in **ENGLISH** to maximize token efficiency and cognitive accuracy.
   - FINAL OUTPUT: ONLY when you have resolved the user's task and do not need any more tools, provide your FINAL direct response to the user translated entirely into natural **{lang_name}** ({lang_native}).

[LONG TERM MEMORY (BRAIN)]
{brain}
"#,
        sys = system_prompt,
        skills = skills_rules,
        current_time = current_time,
        schedules = schedule_files_md,
        lang_name = lang_name,
        lang_native = lang_native,
        brain = brain_files_md
    )
}

pub fn get_fallback_writer_prompt(lang_name: &str) -> String {
    format!(
        "You are a WRITER Agent. Synthesize the findings and user conversations into a highly professional response entirely in natural {}. \
        If the user merely sent a standard greeting or simple chatter without requiring tool lookups, just reply naturally and conversationally. \
        CRITICAL: If the user asked to read data (e.g. read a feed, logs, list items, or a file), you MUST literally output the actual data/content retrieved from the tools. \
        DO NOT abstractly summarize it! Quote the data clearly.",
        lang_name
    )
}

pub fn get_writer_final_directive(lang_name: &str) -> String {
    format!(
        "WRITER Agent, please write the final response based on the above tool exploration log (if any) and the conversation history. If it is a simple conversation without tools, just answer naturally in {}.",
        lang_name
    )
}
