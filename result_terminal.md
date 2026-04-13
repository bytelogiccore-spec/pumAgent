cargo :    Compiling pumagent v0.1.0-beta (D:\ByteLogicCore\AI\PumAgent_Rust\src-tauri)
At line:1 char:1
+ cargo test --test terminal_security_test -- --nocapture > ../result_t ...
+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    + CategoryInfo          : NotSpecified: (   Compiling pu...Rust\src-tauri):String) [], RemoteException
    + FullyQualifiedErrorId : NativeCommandError
 
warning: unused import: `std::path::PathBuf`
 --> tests\terminal_security_test.rs:1:5
  |
1 | use std::path::PathBuf;
  |     ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: `pumagent` (test "terminal_security_test") generated 1 warning (run `cargo fix --test "terminal_security_test"
 -p pumagent` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 58.77s
     Running tests\terminal_security_test.rs (target\debug\deps\terminal_security_test-094b337508d2bc18.exe)

running 1 test
==================================================
🛡️  Terminal Tool Security Sandbox Test 
==================================================
✅ Safe Command Passed: echo 'Hello World'
✅ Dangerous Command Blocked: Remove-Item C:\Windows -Recurse -Force
   => Block reason: Security Error: The command contains dangerous keywords blocked by the system sandbox.
==================================================
✅ Security checks fully passed!
test test_terminal_security_sandbox ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.56s

