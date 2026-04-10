### 💖 프로젝트 후원하기
PumAgent가 유용하다고 생각되신다면 프로젝트 개발을 후원해 주세요!

[<img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="ko-fi" height="36">](https://ko-fi.com/YOUR_KO_FI_LINK)

여러분의 후원은 다음 항목들에 큰 도움이 됩니다:
- 🚀 새로운 기능 추가 및 성능 최적화
- 🐛 버그 수정 및 안정성 향상
- 📚 문서화 및 튜토리얼 제작
- 💻 테스트 인프라 및 CI/CD 유지보수

# PumAgent 🤖

PumAgent는 **Rust (Tauri)** 엔진과 **Svelte / TypeScript** 프론트엔드로 구축된 자율 지능형 데스크톱 애플리케이션입니다. 빠른 속도, 개인정보 보호 및 지속적인 확장을 위해 설계된 PumAgent는 복잡한 에이전트 워크플로우를 오케스트레이션하여 귀하의 로컬 환경에서 직접 실행할 수 있는 개인 AI를 제공합니다.

---

## 🌟 주요 기능

- **자율 에이전트 루프**: 사람의 지속적인 개입 없이 스스로 계획하고 작업을 완수하는 강력한 `목표-추론-행동-관찰-완료(Goal-Reason-Action-Observe-Finish)` 아키텍처.
- **지속성 메모리 및 데이터**: 고성능 `DBX-Core` 로컬 Key-Value 데이터베이스 스토리지로 실시간 데이터 통합 및 지속성을 유지합니다. 방대한 "Brain" 아티팩트와 사용자 정의 지식 기반(Knowledge Base)을 지원합니다.
- **웹 메타 검색 및 크롤링**: Tavily 및 Google Custom Search API를 활용하는 다층 검색 통합 로직. 트래픽 한도 초과 시 DuckDuckGo 및 Yahoo 헤드리스 웹 스크래핑으로 자동 전환됩니다.
- **선제적 텔레그램 통합**: 에이전트를 사용자의 모바일 텔레그램 기기와 직접 연동하여 실시간 푸시 알림, 경고 및 원격 제어 인터랙션을 제공합니다.
- **자동화 스케줄러**: CRON 스케줄을 활용한 안정적인 간격 폴링(Interval Polling) 루프를 통해 에이전트가 백그라운드 환경에서 지속적으로 최신 뉴스, 시장 데이터 및 내부 시스템을 모니터링할 수 있게 합니다.
- **크로스 플랫폼 지원**: 가벼운 Tauri 풋프린트 덕분에 Windows, macOS, Linux 플랫폼에 환경에 구애받지 않고 네이티브로 최적화되었습니다.

## 🛠️ 기술 스택

1. **프론트엔드**: [Svelte 5](https://svelte.dev/) + [Vite](https://vitejs.dev/) + [TypeScript](https://www.typescriptlang.org/)
2. **백엔드**: [Rust](https://www.rust-lang.org/) + [Tauri](https://tauri.app/)
3. **데이터베이스**: 로컬 환경에서 독자적으로 동작하는 `DBX-Core` Key-Value 스토리지
4. **외부 서비스 연동**:
   - Ollama / Text-generation-webui를 활용한 로컬 모델 추론
   - 외부 외부 대형 언어 모델 (동적 LLM 컴포넌트를 통한 OpenAI, Gemini 등 지원)
   - 메타 검색 API 연동 및 네이티브 웹 크롤링 모듈

## 🚀 시작하기

### 사전 요구 사항
- Node.js (v18+)
- Rust & Cargo (1.77.2+)
- Visual Studio / C++ Build Tools (Windows 네이티브 모듈 빌드 환경)

### 설치 방법
1. 저장소를 클론합니다.
   ```bash
   git clone https://github.com/bytelogiccore-spec/pumAgent.git
   cd pumAgent
   ```
2. 프론트엔드 종속성을 설치합니다.
   ```bash
   npm install
   ```
3. 개발 모드로 애플리케이션을 실행합니다.
   ```bash
   npm run tauri dev
   ```

### 프로덕션 빌드
배포를 위한 독립 실행형 데스크톱 애플리케이션으로 컴파일하려면, 다음 명령어를 실행합니다:
```bash
npm run tauri build
```
완료되면 `src-tauri/target/release/bundle` 폴더 위치에 설치 파일 및 독립 실행형 파일이 생성됩니다.

## 📄 라이선스
이 프로젝트는 **MIT License**로 배포됩니다. 자세한 정보는 [LICENSE](LICENSE) 파일을 확인하세요. 제3자 라이선스(Third-Party License) 관련 상세 내용은 [THIRDPARTY-LICENSES.md](THIRDPARTY-LICENSES.md)에서 확인하실 수 있습니다.
