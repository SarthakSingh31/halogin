use std::{io, process::Command};

fn main() -> Result<(), io::Error> {
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/static");
    println!("cargo:rerun-if-changed=frontend/.npmrc");
    println!("cargo:rerun-if-changed=frontend/package-lock.json");
    println!("cargo:rerun-if-changed=frontend/package.json");
    println!("cargo:rerun-if-changed=frontend/postcss.config.cjs");
    println!("cargo:rerun-if-changed=frontend/svelte.config.js");
    println!("cargo:rerun-if-changed=frontend/tailwind.config.cjs");
    println!("cargo:rerun-if-changed=frontend/tsconfig.json");
    println!("cargo:rerun-if-changed=frontend/vite.config.ts");

    Command::new("npm")
        .arg("install")
        .current_dir("./frontend")
        .output()?;
    Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("./frontend")
        .output()?;

    Ok(())
}
