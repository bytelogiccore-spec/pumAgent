cargo :    Compiling pumagent v0.1.0-beta (D:\ByteLogicCore\AI\PumAgent_Rust\src-tauri)
At line:1 char:1
+ cargo test --test tool_calling_test -- --nocapture > ../result.md 2>& ...
+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    + CategoryInfo          : NotSpecified: (   Compiling pu...Rust\src-tauri):String) [], RemoteException
    + FullyQualifiedErrorId : NativeCommandError
 
    Finished `test` profile [unoptimized + debuginfo] target(s) in 11.79s
     Running tests\tool_calling_test.rs (target\debug\deps\tool_calling_test-7dfb6fa599409cf3.exe)

running 1 test
==================================================
🚀 [NEW SCENARIO] Comprehensive Native Tool Calling Test 🚀
목표: LLM이 다양한 유저 요청에 따라 알맞은 Native 툴(crawler, telegram, terminal 등)을 정확히 호출하는지 검증
==================================================

[Scenario 1] Telegram Push Notification
✅ Telegram 툴 콜링 확인 완료.

[Scenario 2] Web Crawler Execution
✅ Crawler 툴 콜링 확인 완료.

[Scenario 3] Terminal CLI Execution
✅ Terminal 툴 콜링 확인 완료.

[Scenario 4] Scripting engine invocation
✅ Scripting 툴 콜링 확인 완료.

==================================================
✅ ALL TOOL CALLING ROUTING SCENARIOS PASSED!
==================================================
test test_comprehensive_native_tool_calling ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 34.07s

