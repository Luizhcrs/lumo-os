//! build.rs — compila C source do plugin libdecor.
//!
//! libdecor plugin ABI e C. Cargo wrappa build via cc::Build pra
//! integrar no workspace.
//!
//! Plugin instalado em /usr/lib/libdecor/plugins-1/libdecor-lumo.so

fn main() {
    println!("cargo:rerun-if-changed=c-src/lumo-decor.c");
    println!("cargo:rerun-if-changed=c-src/draw.c");
    println!("cargo:rerun-if-changed=c-src/draw.h");

    let wayland = pkg_config::probe_library("wayland-client").expect("wayland-client missing");
    let libdecor = pkg_config::probe_library("libdecor-0").expect("libdecor-0 missing");

    let mut build = cc::Build::new();
    build
        .file("c-src/lumo-decor.c")
        .file("c-src/draw.c")
        // SEM -fvisibility=hidden — symbols default attribute em
        // libdecor_plugin_description precisa estar exportado pra
        // libdecor loader achar via dlsym apos cdylib link.
        .flag("-Wall");

    for inc in wayland.include_paths.iter().chain(libdecor.include_paths.iter()) {
        build.include(inc);
    }
    build.compile("lumo_decor_c");

    // Linker tem que puxar libdecor_plugin_description (referenciado so
    // por loader libdecor via dlsym, sem caller estatico em Rust).
    // Uso --undefined pra forcar symbol resolve + --version-script pra
    // explicitar export. Mais robusto que --whole-archive (que tem
    // ordem-dependente entre rustc-link-lib e rustc-link-arg).
    println!("cargo:rustc-link-arg=-Wl,--undefined=libdecor_plugin_description");
    let ver_script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("c-src/lumo-decor.ver");
    println!("cargo:rustc-link-arg=-Wl,--version-script={}", ver_script.display());
    println!("cargo:rerun-if-changed=c-src/lumo-decor.ver");

    for lib in &wayland.libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
}
