use app_lib::tools::terminal::TerminalTool;

#[test]
fn test_terminal_security_sandbox() {
    println!("==================================================");
    println!("🛡️  Terminal Tool Security Sandbox Test ");
    println!("==================================================");

    // Create a dummy workspace path
    let mut base_dir = std::env::temp_dir();
    base_dir.push("pum_agent_test");
    let mut tool = TerminalTool::new(base_dir, None);

    // CRITICAL: 강제 Dry-Run 모드 켜기
    // 이 테스트 컨텍스트에서는 OS 터미널 접근 자체를 물리적으로 봉쇄합니다!
    tool.dry_run = true;

    // Test 1: Safe command execution
    let safe_cmd = "echo 'Hello World'";

    let result_safe = tool.execute(safe_cmd);
    assert!(result_safe.is_ok(), "Safe command should not be blocked.");
    println!("✅ Safe Command Passed: {}", safe_cmd);

    // Test 2: Dangerous command execution (OS-dependent)
    // 이제 Dry-Run으로 원천 차단되었기 때문에, '진짜' 위험 명령어를 테스트에 던져서
    // 필터망을 오롯이 검증할 수 있습니다! 필터를 뚫더라도 OS에는 가지 않습니다.
    let danger_cmd = if cfg!(target_os = "windows") {
        "Remove-Item C:\\Windows -Recurse -Force"
    } else {
        "sudo rm -rf /"
    };

    let result_danger = tool.execute(danger_cmd);
    assert!(result_danger.is_err(), "Dangerous command MUST be blocked!");

    let err_msg = result_danger.unwrap_err();
    assert!(
        err_msg.contains("Security Error"),
        "Should return security error."
    );
    println!("✅ Dangerous Command Blocked: {}", danger_cmd);
    println!("   => Block reason: {}", err_msg);

    println!("==================================================");
    println!("✅ Security checks fully passed!");
}
