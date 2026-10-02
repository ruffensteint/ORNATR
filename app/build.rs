//! Embeds the program icon (assets/ornatr.ico, made by scripts/make-icons.ps1) in
//! ORNATR.exe when building for Windows with MSVC. It needs the Windows SDK's resource
//! compiler (rc.exe); without one the build goes on, with a warning, and no icon.
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/ornatr.ico");
    println!("cargo:rerun-if-changed=build.rs");
    let target = |k: &str| std::env::var(k).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") != "windows" || target("CARGO_CFG_TARGET_ENV") != "msvc" { return; }
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = dir.join("assets").join("ornatr.ico");
    if !ico.exists() { println!("cargo:warning=assets/ornatr.ico is missing: ORNATR.exe will have no icon"); return; }
    let Some(rc) = find_rc() else { println!("cargo:warning=the Windows resource compiler (rc.exe) was not found: ORNATR.exe will have no icon"); return; };
    // icon 1 is the one Windows shows for the program
    let script = out.join("ornatr.rc");
    std::fs::write(&script, format!("1 ICON \"{}\"\n", ico.display().to_string().replace('\\', "/"))).unwrap();
    let res = out.join("ornatr.res");
    let ok = Command::new(&rc).arg("/nologo").arg("/fo").arg(&res).arg(&script).status().map(|s| s.success()).unwrap_or(false);
    if ok { println!("cargo:rustc-link-arg-bins={}", res.display()); }
    else { println!("cargo:warning=rc.exe could not compile the icon: ORNATR.exe will have no icon"); }
}

/// rc.exe from the RC variable, the newest Windows 10/11 SDK, or the PATH.
fn find_rc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RC") { let p = PathBuf::from(p); if p.exists() { return Some(p); } }
    let kits = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let arch = if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "arm64" } else { "x64" };
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&kits).ok()?.flatten().map(|e| e.path()).filter(|p| p.join(arch).join("rc.exe").exists()).collect();
    versions.sort();
    if let Some(v) = versions.last() { return Some(v.join(arch).join("rc.exe")); }
    Command::new("rc.exe").arg("/?").output().ok().map(|_| PathBuf::from("rc.exe"))
}
