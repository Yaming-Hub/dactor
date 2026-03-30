fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure protoc is found via PATH if PROTOC is not set
    if std::env::var("PROTOC").is_err() {
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("protoc.exe")
            .output()
        {
            if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                std::env::set_var("PROTOC", line.trim());
            }
        }
    }
    tonic_build::compile_protos("proto/test_node.proto")?;
    Ok(())
}
