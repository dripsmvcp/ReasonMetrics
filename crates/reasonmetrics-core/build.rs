use std::env;
use std::fs;
use std::path::PathBuf;

// Embeds every registry/*.toml (repo root) into the crate as RAW_ENTRIES so
// the CLI, wasm, and future bindings all ship identical model data. When the
// registry directory is absent (e.g. a packaged crate built outside the
// workspace), RAW_ENTRIES is empty and the library degrades gracefully.
fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let registry = manifest.join("../../registry");
    println!("cargo:rerun-if-changed={}", registry.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&registry)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut out = String::from(
        "/// (file name, raw TOML) for every registry entry, embedded at build time.\n\
         pub static RAW_ENTRIES: &[(&str, &str)] = &[\n",
    );
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy();
        let abs = f.canonicalize().unwrap();
        out.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            name,
            abs.display().to_string()
        ));
    }
    out.push_str("];\n");

    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("registry_gen.rs");
    fs::write(dest, out).unwrap();
}
