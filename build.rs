use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    glib_build_tools::compile_resources(
        &["assets"],
        "assets/khushu.gresource.xml",
        "khushu.gresource",
    );

    println!("cargo:rerun-if-changed=po/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let locale_dir = Path::new(&manifest_dir).join("target").join("locale");

    if let Ok(entries) = fs::read_dir("po") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("po") {
                let file_stem = path.file_stem().unwrap().to_str().unwrap();

                let parts: Vec<&str> = file_stem.split('.').collect();
                let (domain, lang) = if parts.len() == 1 {
                    ("khushu", parts[0])
                } else {
                    (parts[0], parts[1])
                };

                if lang == "pot" || file_stem == "khushu" {
                    continue;
                }

                let lang_dir = locale_dir.join(lang).join("LC_MESSAGES");
                fs::create_dir_all(&lang_dir).unwrap();

                let mo_path = lang_dir.join(format!("{}.mo", domain));

                let status = Command::new("msgfmt")
                    .arg("-o")
                    .arg(&mo_path)
                    .arg(&path)
                    .status();

                if let Ok(status) = status {
                    if !status.success() {
                        println!("cargo:warning=msgfmt failed for {:?}", path);
                    }
                } else {
                    println!("cargo:warning=Failed to execute msgfmt. Is gettext installed?");
                }
            }
        }
    }
}
