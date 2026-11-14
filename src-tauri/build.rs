use std::path::PathBuf;

fn main() {
    // Для запуска из `tauri dev` кладём WinDivert рядом с собранным exe.
    let src = PathBuf::from("../third_party/windivert");
    println!("cargo:rerun-if-changed=../third_party/windivert");
    if let (Ok(out), true) = (std::env::var("OUT_DIR"), src.exists()) {
        // OUT_DIR = <target>/<profile>/build/<crate>/out
        if let Some(profile_dir) = PathBuf::from(out).ancestors().nth(3) {
            for f in ["WinDivert.dll", "WinDivert64.sys"] {
                let from = src.join(f);
                if from.exists() {
                    let _ = std::fs::copy(&from, profile_dir.join(f));
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=proryv.exe.manifest");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("proryv.exe.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build");
}
