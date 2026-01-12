use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    // Only build frontend in release mode
    let profile = env::var("PROFILE").unwrap_or_default();

    if profile == "release" {
        println!("cargo:rerun-if-changed=web/src");
        println!("cargo:rerun-if-changed=web/package.json");
        println!("cargo:rerun-if-changed=web/vite.config.ts");
        println!("cargo:rerun-if-changed=web/tailwind.config.js");

        println!("cargo:warning=Building frontend assets...");

        let web_dir = Path::new("web");

        // Check if web directory exists
        if !web_dir.exists() {
            println!("cargo:warning=web/ directory not found, skipping frontend build");
            return;
        }

        // Check if package.json exists
        if !web_dir.join("package.json").exists() {
            println!("cargo:warning=web/package.json not found, skipping frontend build");
            return;
        }

        // Install dependencies if node_modules doesn't exist
        if !web_dir.join("node_modules").exists() {
            println!("cargo:warning=Installing frontend dependencies...");

            let npm_install = Command::new(if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" })
                .args(["install"])
                .current_dir("web")
                .status()
                .expect("Failed to run npm install");

            if !npm_install.success() {
                panic!("npm install failed");
            }
        }

        // Build frontend
        println!("cargo:warning=Building frontend with Vite...");

        let npm_build = Command::new(if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" })
            .args(["run", "build"])
            .current_dir("web")
            .status()
            .expect("Failed to run npm build");

        if !npm_build.success() {
            panic!("Frontend build failed");
        }

        println!("cargo:warning=Frontend build completed successfully");

        // Verify dist directory was created
        if !web_dir.join("dist").exists() {
            panic!("web/dist/ directory not found after build");
        }
    }
}
