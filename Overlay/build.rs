use std::process::Command;

fn main() {
    // Should trigger if it doesn't exist
    println!("cargo:rerun-if-changed=helper/mumble.exe");
    println!("cargo:rerun-if-changed=helper/mumble_helper.cpp");
    Command::new("make")
        .current_dir("helper")
        .status()
        .expect("Failed to compile mumble helper");
}
