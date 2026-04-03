fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure protoc is found. Check common locations if PROTOC is not set.
    if std::env::var("PROTOC").is_err() {
        // Try chocolatey's install path first (Windows)
        let choco_path = "C:\\ProgramData\\chocolatey\\lib\\protoc\\tools\\bin\\protoc.exe";
        if std::path::Path::new(choco_path).exists() {
            std::env::set_var("PROTOC", choco_path);
        } else if let Ok(output) = std::process::Command::new("where.exe")
            .arg("protoc.exe")
            .output()
        {
            if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    std::env::set_var("PROTOC", trimmed);
                }
            }
        }
    }
    tonic_build::compile_protos("proto/test_node.proto")?;
    Ok(())
}
