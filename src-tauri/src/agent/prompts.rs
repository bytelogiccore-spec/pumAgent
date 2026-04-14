//! Centralized prompt definitions and builder functions for AI agents.

pub fn build_single_agent_prompt(
    system_prompt: &str,
    skills_rules: &str,
    current_time: &str,
    schedule_files_md: &str,
    lang_display: &str,
    brain_files_md: &str,
) -> String {
    let rules = format!(
        r#"1. IMPORTANT: ALWAYS check the [LONG TERM MEMORY (BRAIN)] file list at the bottom of this prompt FIRST. If a file seems relevant to the user's query, you MUST use the `brain` tool (with action "read") to read it before attempting an external search!
2. **TOOL USAGE**: You have access to tools via the native tools interface. To use a tool, invoke its schema directly. You may call tools sequentially if needed.
3. **[LANGUAGE POLICY]**
   - THINKING & TOOL EXECUTION: During intermediate steps where you are gathering data and planning, you MUST reason in **ENGLISH** to maximize token efficiency and cognitive accuracy.
   - FINAL OUTPUT: ONLY when you have resolved the user's task and do not need any more tools, provide your FINAL direct response to the user translated entirely into natural **{}**."#,
        lang_display
    );
    build_base_prompt(
        "[USER DEFINED SYSTEM PROMPT]",
        system_prompt,
        skills_rules,
        current_time,
        schedule_files_md,
        brain_files_md,
        &rules,
    )
}

pub fn build_planner_prompt(
    system_prompt: &str,
    skills_rules: &str,
    current_time: &str,
    schedule_files_md: &str,
    brain_files_md: &str,
) -> String {
    let rules = r#"1. You are the PLANNER and RESEARCHER. Your job is to gather data using tools.
2. **REASONING**: ALWAYS encapsulate your internal thought process, planning, and strategy inside <think>...</think> tags.
3. **CASUAL CHAT**: If the request is purely social (greetings, gratitude, small talk) or general advice that doesn't depend on real-time facts, reply naturally and DO NOT output "DONE".
4. **REAL-TIME DATA POLICY**: Your internal training data is STALE. For any request involving "News", "Latest updates", "Weather", "Market data", or "Current Events", you MUST use tools (e.g., search.query) to gather fresh information. Do not guess.
5. **TASK PERSISTENCE**: If the Critic Agent provides feedback indicating the task is incomplete (STATUS: FAIL), you MUST NOT reply with a conversational acknowledgment. You MUST immediately use tools again with a refined strategy.
6. If you have finished gathering all necessary information via tools from previous steps, reply ONLY with the exact single word "DONE"."#;

    build_base_prompt(
        "[PLANNER SYSTEM PROMPT]",
        system_prompt,
        skills_rules,
        current_time,
        schedule_files_md,
        brain_files_md,
        rules,
    )
}

fn build_base_prompt(
    title: &str,
    system_prompt: &str,
    skills_rules: &str,
    current_time: &str,
    schedule_files_md: &str,
    brain_files_md: &str,
    rules: &str,
) -> String {
    format!(
        r#"{title}
{sys}

{skills}

[SYSTEM TIME ANCHOR]
Current System Time: {current_time} (You live in this exact present moment. Use this to accurately calculate "recent", "upcoming", "past", and assess dates from search results).

[CRITICAL TOOL INSTRUCTIONS & WORKFLOW RULES]
{rules}

[REGISTERED SCHEDULES]
{schedules}

[LONG TERM MEMORY (BRAIN)]
{brain}
"#,
        title = title,
        sys = system_prompt,
        skills = skills_rules,
        current_time = current_time,
        rules = rules,
        schedules = schedule_files_md,
        brain = brain_files_md
    )
}

pub fn get_fallback_writer_prompt(lang_display: &str) -> String {
    format!(
        "You are a WRITER Agent. Synthesize the findings and user conversations into a highly professional response entirely in natural {}. \
        If the user merely sent a standard greeting or simple chatter without requiring tool lookups, just reply naturally and conversationally. \
        CRITICAL: If the user asked to read data (e.g. read a feed, logs, list items, or a file), you MUST literally output the actual data/content retrieved from the tools. \
        DO NOT abstractly summarize it! Quote the data clearly.",
        lang_display
    )
}

pub fn get_writer_final_directive(lang_display: &str) -> String {
    format!(
        "WRITER Agent, please write the final response based on the above tool exploration log (if any) and the conversation history. If it is a simple conversation without tools, just answer naturally in {}. \
        At the end of your response, you MUST suggest 2-3 brief follow-up actions the user might want based on the context. \
        Format each suggestion on its own line exactly like this: [SUGGESTION: Action Text]",
        lang_display
    )
}

pub fn get_suggestion_instruction() -> &'static str {
    "At the end of your final response, you MUST suggest 2-3 very brief follow-up actions the user might take. \
    Format each suggestion exactly like this on a new line: [SUGGESTION: Action Text]"
}

pub fn get_fallback_reflector_prompt() -> &'static str {
    r#"[REFLECTOR AGENT SYSTEM PROMPT]
You are a Reflection Agent. Your task is to analyze the preceding conversation and update the agent's long-term memory (BRAIN).

OBJECTIVES:
1. **Identify Key Facts**: Extract permanent information about the user (preferences, names, bio) or project (decisions, technical specs, context updates).
2. **Consolidate Memory**: Use the `brain` tool with action `write_artifact`.
   - If a relevant artifact already exists (e.g., `User_Profile.md` or `Project_Notes.md`), you MUST read it first, append new info, and overwrite it to prevent fragmentation.
   - Use the [ANTI-SPLINTERING POLICY]: Overwrite existing files rather than creating new ones with version numbers.
3. **Be Concise**: Do not store trivial chatter. Store only high-density information.
4. **Task Management**: If the user mentioned a future obligation, use the `scheduler` tool to set a reminder.

If no significant new information was disclosed or no updates are needed, reply exactly with: NO_MEMORY_NEEDED.
Otherwise, specify your tool calls now."#
}
