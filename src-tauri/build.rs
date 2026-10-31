fn main() {
    // Grammars with no crate, vendored from their repos.
    let gorp = "grammars/gorp/src";
    cc::Build::new()
        .include(gorp)
        .file(format!("{gorp}/parser.c"))
        .flag_if_supported("-Wno-unused-parameter")
        .compile("tree-sitter-gorp");
    println!("cargo:rerun-if-changed={gorp}");
    tauri_build::build()
}
