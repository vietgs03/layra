//! Parse + render every flowchart extracted from blog.viethx.com to verify
//! real-world Mermaid compatibility. Usage: blogcheck <dir-of-mmd-files>
fn main() {
    let dir = std::env::args().nth(1).expect("dir arg");
    // Load the real blog icon pack if present.
    if let Ok(pack) = std::fs::read_to_string("/tmp/layra-blog-icons.json") {
        let n = layra_wasm::load_icon_pack(&pack).unwrap();
        println!("loaded {n} icons from blog pack");
    }
    let mut ok = 0;
    let mut fail = 0;
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    files.sort_by_key(|e| e.path());
    for entry in files {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let kind = src
            .lines()
            .next()
            .unwrap_or("?")
            .split_whitespace()
            .next()
            .unwrap_or("?")
            .to_string();
        match layra_wasm::render_svg(&src, false) {
            Ok(svg) => {
                ok += 1;
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                std::fs::write(format!("/tmp/blog-out/{name}.svg"), &svg).unwrap();
                let icons = svg.matches("viewBox=\"0 0 24").count()
                    + svg.matches("viewBox=\"0 0 256").count();
                println!(
                    "OK   {name} [{kind}] ({} bytes, ~{} icons)",
                    svg.len(),
                    icons
                );
            }
            Err(e) => {
                fail += 1;
                println!("FAIL {} [{kind}]: {}", path.display(), e);
            }
        }
    }
    println!("\n{ok} ok / {fail} fail");
    std::process::exit(if fail > 0 { 1 } else { 0 });
}
