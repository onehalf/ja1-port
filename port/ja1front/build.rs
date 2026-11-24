//! Build script: the port commit for the recorder's meta.txt (record.rs). Reads .git with std::fs only.
use std::path::Path;

fn main() {
    let git = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git");
    let head = git.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    let mut commit = "unknown".to_string();
    if let Ok(h) = std::fs::read_to_string(&head) {
        let h = h.trim();
        if let Some(r) = h.strip_prefix("ref: ") {
            let f = git.join(r);
            println!("cargo:rerun-if-changed={}", f.display());
            if let Ok(c) = std::fs::read_to_string(&f) {
                commit = c.trim().to_string();
            } else if let Ok(p) = std::fs::read_to_string(git.join("packed-refs")) {
                if let Some(l) = p.lines().find(|l| l.ends_with(&format!(" {}", r))) {
                    commit = l.split(' ').next().unwrap_or("unknown").to_string();
                }
            }
        } else {
            commit = h.to_string();
        }
    }
    println!("cargo:rustc-env=JA1_COMMIT={}", commit);
}
