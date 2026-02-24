use std::process::Command;

fn main() {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--release")
        .output()
        .expect("Failed to execute process");

    let result = String::from_utf8_lossy(&output.stdout);
    println!("Ran output");
}
