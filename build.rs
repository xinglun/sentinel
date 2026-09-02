use std::fs;
use std::path::PathBuf;

fn main() {
    let proto_dir = PathBuf::from("src/adapters/futu/proto");

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!(
        "cargo:rustc-env=SENTINEL_BUILD_GIT_SHA={}",
        git_value(&["rev-parse", "HEAD"])
    );
    println!(
        "cargo:rustc-env=SENTINEL_BUILD_GIT_BRANCH={}",
        git_value(&["branch", "--show-current"])
    );

    // Watch for changes in the proto directory
    println!("cargo:rerun-if-changed={}", proto_dir.display());

    // Find all .proto files
    let mut proto_files = Vec::new();
    if proto_dir.exists() {
        for entry in fs::read_dir(&proto_dir).expect("Failed to read proto dir") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("proto") {
                proto_files.push(path);
            }
        }
    }

    if proto_files.is_empty() {
        println!(
            "cargo:warning=CRITICAL: No proto files found in {}. Did you clone the repository?",
            proto_dir.display()
        );
    } else {
        println!(
            "Compiling {} proto files into src/adapters/futu/protocol/generated",
            proto_files.len()
        );
        // Compile them into Rust traits using prost-build
        prost_build::Config::new()
            .out_dir("src/adapters/futu/protocol/generated")
            .compile_protos(&proto_files, &[&proto_dir])
            .unwrap_or_else(|e| panic!("Failed to compile protos: {}", e));
    }
}

fn git_value(arguments: &[&str]) -> String {
    std::process::Command::new("git")
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}
