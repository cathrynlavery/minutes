use std::path::PathBuf;
use std::process::Command;

fn main() {
    compile_system_audio_helper();
    tauri_build::build()
}

fn compile_system_audio_helper() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set"),
    );
    let source = manifest_dir.join("src/system_audio_record.swift");
    let bin_dir = manifest_dir.join("bin");
    let binary = bin_dir.join("system_audio_record");
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let target_binary = bin_dir.join(format!("system_audio_record-{}", target));

    println!("cargo:rerun-if-changed={}", source.display());
    std::fs::create_dir_all(&bin_dir).expect("failed to create helper bin dir");

    let output = Command::new("swiftc")
        .args(["-parse-as-library"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("failed to run swiftc for system_audio_record");

    if !output.status.success() {
        panic!(
            "failed to compile system_audio_record.swift: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    std::fs::copy(&binary, &target_binary)
        .expect("failed to copy target-specific system_audio_record helper");
}
