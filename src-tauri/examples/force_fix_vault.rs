use keyring::Entry;
use std::fs;
use std::path::PathBuf;

fn main() {
    // 1. 여기에 본인의 몰트북 키(moltbook_sk_...)를 넣으세요
    let my_key = "moltbook_sk_u3sR1ahhGu2Ig7e637OuHLiayeqlL9jH";

    if my_key.contains("여기에_") {
        println!("❌ 에러: 키를 먼저 입력해주세요!");
        return;
    }

    let service = "PumAgentVault";
    let key_name = "moltbook_creds";

    println!("⚙️ [{}] 서비스에 [{}] 키 등록 중...", service, key_name);

    // 2. Windows Vault에 직접 쓰기
    let entry = Entry::new(service, key_name).expect("Failed to create entry");
    match entry.set_password(my_key) {
        Ok(_) => println!("✅ 성공: Windows 자격 증명 관리자에 키가 등록되었습니다."),
        Err(e) => {
            println!("❌ 실패: Vault 쓰기 작업 중 에러 발생: {}", e);
            return;
        }
    }

    // 3. 인덱스 파일 업데이트 (D:/ByteLogicCore/AI/PumAgentData/vault_keys.json)
    let data_dir = PathBuf::from("d:/ByteLogicCore/AI/PumAgentData");
    let index_file = data_dir.join("vault_keys.json");
    
    let mut keys = if index_file.exists() {
        let content = fs::read_to_string(&index_file).unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str::<Vec<String>>(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    if !keys.contains(&key_name.to_string()) {
        keys.push(key_name.to_string());
        if let Ok(data) = serde_json::to_string(&keys) {
            let _ = fs::write(index_file, data);
            println!("💾 성공: vault_keys.json 인덱스 파일이 업데이트되었습니다.");
        }
    } else {
        println!("ℹ️ 알림: 인덱스 파일에 이미 항목이 존재합니다.");
    }

    println!("\n🚀 이제 'cargo test --test scenario_test'를 다시 실행해보세요!");
}
