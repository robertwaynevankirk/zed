#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" || target.contains("windows") {
        shader_compilation::compile_shaders();
    }
}

mod shader_compilation {
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        process::{self, Command},
    };

    pub fn compile_shaders() {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let shader_path = manifest_dir.join("src/shaders.hlsl");
        let precompiled_path = manifest_dir.join("src/shaders_bytes_precompiled.rs");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let rust_binding_path = format!("{}/shaders_bytes.rs", out_dir);

        println!("cargo:rerun-if-changed={}", shader_path.display());
        println!("cargo:rerun-if-changed={}", precompiled_path.display());

        let fxc_path = find_fxc_compiler();
        if let Some(fxc_path) = fxc_path {
            let modules = [
                "quad",
                "shadow",
                "path_rasterization",
                "path_sprite",
                "underline",
                "monochrome_sprite",
                "subpixel_sprite",
                "polychrome_sprite",
            ];

            if Path::new(&rust_binding_path).exists() {
                fs::remove_file(&rust_binding_path)
                    .expect("Failed to remove existing Rust binding file");
            }
            for module in modules {
                compile_shader_for_module(
                    module,
                    &out_dir,
                    &fxc_path,
                    shader_path.to_str().unwrap(),
                    &rust_binding_path,
                );
            }

            {
                let shader_path = manifest_dir.join("src/color_text_raster.hlsl");
                compile_shader_for_module(
                    "emoji_rasterization",
                    &out_dir,
                    &fxc_path,
                    shader_path.to_str().unwrap(),
                    &rust_binding_path,
                );
            }
        } else if precompiled_path.exists() {
            println!("cargo:warning=fxc.exe not found, using precompiled HLSL shaders");
            fs::copy(&precompiled_path, &rust_binding_path)
                .expect("Failed to copy precompiled shaders");
        } else {
            panic!("fxc.exe not found and precompiled shaders are missing");
        }
    }

    pub fn find_latest_windows_sdk_binary(
        binary: &str,
    ) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
        #[cfg(target_os = "windows")]
        {
            let key = windows_registry::LOCAL_MACHINE
                .open("SOFTWARE\\WOW6432Node\\Microsoft\\Microsoft SDKs\\Windows\\v10.0")?;

            let install_folder: String = key.get_string("InstallationFolder")?;
            let install_folder_bin = Path::new(&install_folder).join("bin");

            let mut versions: Vec<_> = std::fs::read_dir(&install_folder_bin)?
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect();

            versions.sort_by_key(|s| {
                s.split('.')
                    .filter_map(|p| p.parse().ok())
                    .collect::<Vec<u32>>()
            });

            let arch = match std::env::consts::ARCH {
                "x86_64" => "x64",
                "aarch64" => "arm64",
                _ => Err(format!(
                    "Unsupported architecture: {}",
                    std::env::consts::ARCH
                ))?,
            };

            if let Some(highest_version) = versions.last() {
                return Ok(Some(
                    install_folder_bin
                        .join(highest_version)
                        .join(arch)
                        .join(binary),
                ));
            }
        }

        Ok(None)
    }

    fn find_fxc_compiler() -> Option<String> {
        if let Ok(path) = std::env::var("GPUI_FXC_PATH")
            && Path::new(&path).exists()
        {
            return Some(path);
        }

        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("fxc.exe")
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout);
            return Some(path.trim().to_string());
        }

        if let Ok(output) = std::process::Command::new("which")
            .arg("fxc")
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout);
            return Some(path.trim().to_string());
        }

        if let Ok(Some(path)) = find_latest_windows_sdk_binary("fxc.exe") {
            return Some(path.to_string_lossy().into_owned());
        }

        None
    }

    fn compile_shader_for_module(
        module: &str,
        out_dir: &str,
        fxc_path: &str,
        shader_path: &str,
        rust_binding_path: &str,
    ) {
        let output_file = format!("{}/{}_vs.h", out_dir, module);
        let const_name = format!("{}_VERTEX_BYTES", module.to_uppercase());
        let entry_point = format!("{}_vertex", module);
        let mut child = Command::new(fxc_path)
            .args([
                "/T",
                "vs_5_0",
                "/Vn",
                &const_name,
                "/Fh",
                &output_file,
                "/E",
                &entry_point,
                shader_path,
            ])
            .spawn()
            .expect("Failed to spawn fxc.exe");

        let status = child.wait().expect("Failed to compile shader");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }

        let output_file = format!("{}/{}_ps.h", out_dir, module);
        let const_name = format!("{}_PIXEL_BYTES", module.to_uppercase());
        let entry_point = format!("{}_pixel", module);
        let mut child = Command::new(fxc_path)
            .args([
                "/T",
                "ps_5_0",
                "/Vn",
                &const_name,
                "/Fh",
                &output_file,
                "/E",
                &entry_point,
                shader_path,
            ])
            .spawn()
            .expect("Failed to spawn fxc.exe");

        let status = child.wait().expect("Failed to compile shader");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }

        let vs_bytes = fs::read_to_string(format!("{}/{}_vs.h", out_dir, module))
            .expect("Failed to read vertex shader bytes");
        let ps_bytes = fs::read_to_string(format!("{}/{}_ps.h", out_dir, module))
            .expect("Failed to read pixel shader bytes");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(rust_binding_path)
            .expect("Failed to open Rust binding file");

        let vs_bytes_str = vs_bytes
            .lines()
            .skip(4)
            .take_while(|line| !line.starts_with("#endif"))
            .collect::<Vec<_>>()
            .join("\n");

        let ps_bytes_str = ps_bytes
            .lines()
            .skip(4)
            .take_while(|line| !line.starts_with("#endif"))
            .collect::<Vec<_>>()
            .join("\n");

        file.write_all(vs_bytes_str.as_bytes())
            .expect("Failed to write vertex shader bytes to Rust binding file");
        file.write_all(b"\n")
            .expect("Failed to write newline to Rust binding file");
        file.write_all(ps_bytes_str.as_bytes())
            .expect("Failed to write pixel shader bytes to Rust binding file");
        file.write_all(b"\n")
            .expect("Failed to write newline to Rust binding file");
    }
}
