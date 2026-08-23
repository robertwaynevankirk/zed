#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    println!("cargo::rustc-check-cfg=cfg(gles)");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        #[cfg(feature = "windows-manifest")]
        embed_resource();
    }
}

#[cfg(feature = "windows-manifest")]
fn embed_resource() {
    let manifest = std::path::Path::new("resources/windows/gpui.manifest.xml");
    let rc_file = std::path::Path::new("resources/windows/gpui.rc");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", rc_file.display());
    
    let manifest_abs = std::fs::canonicalize(manifest).unwrap_or_else(|_| manifest.to_path_buf());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let temp_rc = out_dir.join("gpui_generated.rc");
    let manifest_escaped = manifest_abs.to_string_lossy().replace('\\', "/");
    let rc_content = format!("#define RT_MANIFEST 24\n1 RT_MANIFEST \"{}\"\n", manifest_escaped);
    std::fs::write(&temp_rc, rc_content).unwrap();

    embed_resource::compile(&temp_rc, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
