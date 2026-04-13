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
2. If you do not need tools (e.g., simple greetings, casual chat), simply reply naturally to the user and DO NOT output "DONE".
3. If you have finished gathering all necessary information via tools from previous steps, reply ONLY with the exact single word "DONE"."#;
    
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
        "WRITER Agent, please write the final response based on the above tool exploration log (if any) and the conversation history. If it is a simple conversation without tools, just answer naturally in {}.",
        lang_display
    )
}
