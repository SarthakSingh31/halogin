use std::{io, process::Command};

fn main() -> Result<(), io::Error> {
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/static");
    println!("cargo:rerun-if-changed=frontend/package.json");

    Command::new("npm")
        .arg("install")
        .current_dir("./frontend")
        .output()?;
    let output = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("./frontend")
        .output()?;

    assert!(output.status.success());

    Ok(())
}
