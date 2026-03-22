fn main() {
    // Expose APP_VERSION to the crate via env!("APP_VERSION").
    // In CI the workflow sets APP_VERSION to the datever string, e.g. 2026-03-19-0429-a1b2c3d.
    // For local builds a fallback of "dev" is used.
    let app_version = std::env::var("APP_VERSION").unwrap_or_else(|_| "dev".to_string());
    println!("cargo:rustc-env=APP_VERSION={app_version}");
    println!("cargo:rerun-if-env-changed=APP_VERSION");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target = std::env::var("TARGET").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    println!("cargo:rerun-if-changed=assets/icon/PACT.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let mut res = winres::WindowsResource::new();
    if target_env == "gnu" {
        if let Ok(path) = std::env::var("WINDRES") {
            if !path.trim().is_empty() {
                res.set_windres_path(&path);
            }
        } else if target.contains("x86_64-pc-windows-gnu") {
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
    }

    res.set_icon("assets/icon/PACT.ico");
    res.set_manifest(
        r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#,
    );
    res.compile().expect("failed to compile Windows resources");

    if target_env == "gnu" {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        println!("cargo:rustc-link-search=native={out_dir}");
        println!("cargo:rustc-link-arg-bin=process_affinity_control_tool=-Wl,--whole-archive");
        println!("cargo:rustc-link-arg-bin=process_affinity_control_tool=-lresource");
        println!("cargo:rustc-link-arg-bin=process_affinity_control_tool=-Wl,--no-whole-archive");
    }
}
