use std::process;

fn main() {
    make_build_info();
}

include!("src/build_info.rs");
fn make_build_info() {
    {
        let build_time = chrono::Utc::now().timestamp_micros();
        println!("cargo:rustc-env={}={}", BUILD_TIME, build_time);
    }
    {
        let version = env!("CARGO_PKG_VERSION");
        println!("cargo:rustc-env={}={}", VERSION, version);
    }
    {
        let git_hash = process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        println!("cargo:rustc-env={}={}", GIT_HASH, git_hash);
    }
}
