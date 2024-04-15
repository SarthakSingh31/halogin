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

    let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
        panic!("Failed to format stdout");
    };
    let Ok(stderr) = std::str::from_utf8(&output.stderr) else {
        panic!("Failed to format stderr");
    };

    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );

    Ok(())
}
