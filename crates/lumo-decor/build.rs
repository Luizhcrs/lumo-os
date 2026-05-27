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

    // Symbols do .a sao puxados via --whole-archive pra preservar
    // libdecor_plugin_description (nao referenciado por Rust mas
    // necessario pro loader libdecor).
    println!("cargo:rustc-link-arg=-Wl,--whole-archive");
    println!("cargo:rustc-link-arg=-llumo_decor_c");
    println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");

    for lib in &wayland.libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
    // Forca export explicito do symbol que loader procura via dlsym.
    println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=libdecor_plugin_description");
}
