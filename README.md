### 💖 Support This Project
If you find PumAgent useful, please consider supporting its development!

[<img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="ko-fi" height="36">](https://ko-fi.com/YOUR_KO_FI_LINK)

Your support helps with:
- 🚀 Bringing even smarter autonomous features to life
- 🐛 Squashing bugs and keeping the agent blazingly fast
- 📚 Creating awesome tutorials so anyone can build their own agent
- 💻 Maintaining the open-source infrastructure

---

# PumAgent 🤖 Your Personal AI Sidekick

**Imagine having an AI that doesn't just sit and wait for your chat messages.** 

What if your AI woke up on its own, checked the morning news, remembered your favorite topics from three weeks ago, texted your phone on Telegram with an urgent alert, and autonomously surfed the web—all while running as a lightning-fast native desktop app?

Meet **PumAgent**. It's not just another ChatGPT clone. It is a highly capable, autonomous AI agent that truly *lives* on your machine. 

---

## ✨ Why PumAgent Will Blow Your Mind

- **🧠 It Never Forgets (True Long-Term Memory)**
  PumAgent features a persistent "Brain" powered by a locally crafted, blazing-fast `DBX-Core` database. Tell it your preferences once, and it remembers them forever. No more repeating yourself!

- **⏰ It Thinks While You Sleep (True Autonomy)**
  Want your AI to check cryptocurrency prices every hour? Easy. PumAgent utilizes powerful background schedules. Give it a task, and it will execute the Goal-Reason-Action loop completely on its own, anytime, day or night.

- **📱 It Texts You When It Matters (Proactive Telegram)**
  Why keep checking the app? Bind PumAgent to your Telegram! If the agent discovers something important in the background, it will fire a push notification straight to your phone. It's like having a real personal assistant.

- **🪄 Zero-Dependency "Magic" Scripting**
  PumAgent comes with a deeply embedded, ultra-lightweight `Rhai` scripting engine. This means your AI can magically write and execute its own native plugins on the fly (like communicating with APIs or parsing data) without you needing to install Python, Node.js, or messy OS scripts!

- **⚡ Blazing Fast & Cross-Platform**
  Built on a powerful Rust backend and Tauri, PumAgent uses a fraction of the RAM compared to standard Electron apps. Whether you are on Windows, macOS, or Linux, it feels native, snappy, and secure.

---

## 🛠️ The Geeky Stuff (Tech Stack)

We chose the absolute best tools for performance and safety:
1. **Frontend**: Svelte 5 + Vite + TypeScript (Incredibly responsive UI)
2. **Backend Engine**: Rust + Tauri (Safe, native speeds)
3. **Storage Engine**: `DBX-Core` (Custom, zero-latency Key-Value local storage)
4. **AI Powers**: Supports Local Inference (Ollama) and Dynamic External LLMs (OpenAI, Gemini)

---

## 🚀 Bring Your Agent to Life

Ready to start? You only need a few things to fire up PumAgent on your local machine.

### Prerequisites
- Node.js (v18+)
- Rust & Cargo (1.77.2+)
- Visual Studio / C++ Build Tools (For Windows native modules)

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
To compile the standalone desktop application and share it, run:
```bash
npm run tauri build
```
Once complete, your installer will be waiting for you inside the `src-tauri/target/release/bundle` directory!

---

## 📄 License
This project is licensed under the **MIT License**. Check the [LICENSE](LICENSE) file for more information. Details regarding Third-Party Licenses can be found in [THIRDPARTY-LICENSES.md](THIRDPARTY-LICENSES.md).
