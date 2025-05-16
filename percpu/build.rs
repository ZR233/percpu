use std::path::Path;

fn main() {
    if cfg!(target_os = "linux") && cfg!(not(feature = "sp-naive")) {
        let name = if cfg!(feature = "custom-tp") {
            "test_percpu.c.x"
        } else {
            "test_percpu.x"
        };

        let ld_script_path = Path::new(std::env!("CARGO_MANIFEST_DIR")).join(name);
        println!("cargo:rustc-link-arg-tests=-no-pie");
        println!("cargo:rustc-link-arg-tests=-T{}", ld_script_path.display());
    }
}
