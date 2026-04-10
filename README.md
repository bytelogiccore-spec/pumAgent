### 💖 Support This Project
If you find PumAgent useful, please consider supporting its development!

[<img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="ko-fi" height="36">](https://ko-fi.com/YOUR_KO_FI_LINK)

Your support helps with:
- 🚀 New features and performance optimizations
- 🐛 Bug fixes and stability improvements
- 📚 Documentation and tutorials
- 💻 Test infrastructure and CI/CD maintenance

# PumAgent 🤖

PumAgent is an autonomous intelligence desktop application built with a **Rust (Tauri)** engine and a **Svelte / TypeScript** frontend. Designed for speed, privacy, and continuous capability, PumAgent orchestrates complex agentic workflows, empowering you with a personal AI that can run directly on your machine.

---

## 🌟 Key Features

- **Autonomous Agent Loop**: Robust `Goal-Reason-Action-Observe-Finish` architecture that guarantees the completion of tasks without requiring constant human intervention.
- **Persistent Memory & Data**: Real-time integration and persistence via local SQLite backed by our highly robust `DBX-Core` database engine. Supports extensive "Brain" artifacts and customizable Knowledge Bases.
- **Web Meta-Search & Scraping**: Multi-layered search integration utilizing APIs from Tavily and Google Custom Search, falling back to seamless DuckDuckGo & Yahoo headless web scraping when limits are reached.
- **Proactive Telegram Integration**: Bind the agent directly to your Telegram device for real-time mobile push notifications, alerts, and interaction on-the-go.
- **Automated Scheduler**: Robust interval polling and chronicler loops utilizing CRON schedules, allowing the agent to continuously monitor news, market data, and internal systems in the background.
- **Cross-Platform Readiness**: Designed and optimized natively for Windows, macOS, and Linux thanks to the lightweight Tauri footprint.

## 🛠️ Technology Stack

1. **Frontend**: [Svelte 5](https://svelte.dev/) + [Vite](https://vitejs.dev/) + [TypeScript](https://www.typescriptlang.org/)
2. **Backend**: [Rust](https://www.rust-lang.org/) + [Tauri](https://tauri.app/)
3. **Database**: SQLite powered locally by `DBX-Core`
4. **Third-Party Providers**:
   - Local Inference via Ollama / Text-generation-webui
   - External LLMs (OpenAI, Gemini Support via dynamic LLM client)
   - Search Meta-APIs & Native Web Crawling

## 🚀 Getting Started

### Prerequisites
- Node.js (v18+)
- Rust & Cargo (1.77.2+)
- Visual Studio / C++ Build Tools (Specifically for Windows native modules)

### Installation
1. Clone the repository
   ```bash
   git clone https://github.com/bytelogiccore-spec/pumAgent.git
   cd pumAgent
   ```
2. Install frontend dependencies
   ```bash
   npm install
   ```
3. Run the application in development mode
   ```bash
   npm run tauri dev
   ```

### Building for Production
To compile the standalone desktop application, run:
```bash
npm run tauri build
```
Once complete, your installer or executable will be available within the `src-tauri/target/release/bundle` directory.

## 📄 License
This project is licensed under the **MIT License**. Check the [LICENSE](LICENSE) file for more information. Details regarding Third-Party Licenses can be found in [THIRDPARTY-LICENSES.md](THIRDPARTY-LICENSES.md).
