use std::env;
use std::fs;

fn main() {
    let out_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "target/fixtures/hello".to_string());

    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        fs::create_dir_all(parent).expect("failed to create output directory");
    }

    let bytes = mkfixture::build_hello();
    fs::write(&out_path, &bytes).expect("failed to write fixture");

    eprintln!(
        "wrote {} bytes to {out_path} (entry vmaddr = {:#x})",
        bytes.len(),
        mkfixture::expected_entry_vmaddr()
    );
}
