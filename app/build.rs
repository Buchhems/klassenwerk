// Build steps for Windows:
// 1. Program icon and version information go into Klassenwerk.exe
//    (prepared files in assets/windows, made by packaging/windows/make-resources.sh).
//    Set KLASSENWERK_NO_ICON=1 to build without them.
// 2. The GNU Windows toolchain on some machines ships without an import library for
//    shlwapi (needed by the link-opening helper inside the UI library). GNU ld can link
//    straight against the system DLL, so it is copied into the build output folder and
//    that folder is added to the library search path.
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KLASSENWERK_NO_ICON");
    if !target.contains("windows") {
        return;
    }

    if std::env::var_os("KLASSENWERK_NO_ICON").is_none() {
        let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("windows");
        // MSVC's linker takes the compiled resources directly, GNU ld the COFF object
        let file = if target.contains("msvc") { "klassenwerk.res" } else { "klassenwerk.obj" };
        let path = dir.join(file);
        if path.exists() {
            println!("cargo:rustc-link-arg-bins={}", path.display());
            println!("cargo:rerun-if-changed={}", path.display());
        } else {
            println!("cargo:warning=Windows-Ressourcen fehlen ({file}); die .exe bekommt kein Symbol.");
        }
    }

    if target.contains("windows-gnu") {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("syslibs");
        let _ = std::fs::create_dir_all(&out);
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let dst = out.join("shlwapi.dll");
        if !dst.exists() {
            let _ = std::fs::copy(format!("{windir}\\System32\\shlwapi.dll"), &dst);
        }
        println!("cargo:rustc-link-search=native={}", out.display());
    }
}
