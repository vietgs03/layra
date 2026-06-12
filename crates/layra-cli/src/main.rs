//! `layra` — render Mermaid-compatible diagrams to SVG from the terminal.
//!
//! ```text
//! layra render diagram.mmd                  # writes diagram.svg
//! layra render diagram.mmd -o out.svg --dark
//! cat diagram.mmd | layra render - -o -     # stdin → stdout
//! layra render docs/**.mmd --check          # CI: parse-only, exit 1 on errors
//! layra icons pack.json                     # load extra Iconify packs
//! ```
//!
//! Zero async, zero arg-parser dependency: the surface is small enough
//! that hand-rolled parsing stays clearer than a clap tree.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
layra — Mermaid-compatible diagram renderer (Rust)

USAGE:
  layra render <input.mmd>... [-o <out.svg>] [--dark] [--check] [--icons <pack.json>]...
  layra --version
  layra --help

ARGS:
  <input.mmd>      One or more diagram files; `-` reads stdin.

OPTIONS:
  -o <path>        Output path (single input only); `-` writes stdout.
                   Default: input path with .svg extension.
  --dark           Render with the dark theme.
  --check          Parse + render but write nothing; exit 1 on any error.
                   Lenient warnings (skipped lines) are reported and fail
                   the check too — CI wants the strict view.
  --icons <path>   Load an Iconify-format icon pack (repeatable).
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("render") => render_cmd(&args[1..]),
        Some("--version" | "-V") => {
            println!("layra {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown command '{other}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

struct RenderOpts {
    inputs: Vec<String>,
    output: Option<String>,
    dark: bool,
    check: bool,
}

fn render_cmd(args: &[String]) -> ExitCode {
    let mut opts = RenderOpts {
        inputs: Vec::new(),
        output: None,
        dark: false,
        check: false,
    };

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-o" | "--output" => match it.next() {
                Some(path) => opts.output = Some(path.clone()),
                None => return fail("-o requires a path"),
            },
            "--dark" => opts.dark = true,
            "--check" => opts.check = true,
            "--icons" => match it.next() {
                Some(path) => {
                    let json = match std::fs::read_to_string(path) {
                        Ok(j) => j,
                        Err(e) => return fail(&format!("cannot read icon pack {path}: {e}")),
                    };
                    if let Err(e) = layra_wasm::load_icon_pack(&json) {
                        return fail(&format!("invalid icon pack {path}: {e}"));
                    }
                }
                None => return fail("--icons requires a path"),
            },
            _ if arg.starts_with('-') && arg != "-" => {
                return fail(&format!("unknown flag '{arg}'"));
            }
            _ => opts.inputs.push(arg.clone()),
        }
    }

    if opts.inputs.is_empty() {
        return fail("no input files (use `-` for stdin)");
    }
    if opts.output.is_some() && opts.inputs.len() > 1 {
        return fail("-o only works with a single input");
    }

    let mut failures = 0usize;
    for input in &opts.inputs {
        if let Err(message) = process_one(input, &opts) {
            eprintln!("error: {input}: {message}");
            failures += 1;
        }
    }

    if failures > 0 {
        eprintln!("{failures} of {} failed", opts.inputs.len());
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn process_one(input: &str, opts: &RenderOpts) -> Result<(), String> {
    let source = if input == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| e.to_string())?;
        buf
    } else {
        std::fs::read_to_string(input).map_err(|e| e.to_string())?
    };

    let (svg, warnings) = layra_wasm::render_svg_lenient(&source, opts.dark)?;

    for w in &warnings {
        eprintln!("warning: {input}: {w}");
    }
    if opts.check {
        // CI mode: warnings are failures, nothing is written.
        return if warnings.is_empty() {
            Ok(())
        } else {
            Err(format!("{} skipped line(s)", warnings.len()))
        };
    }

    let out_path = match &opts.output {
        Some(o) => o.clone(),
        None if input == "-" => "-".to_string(),
        None => default_output(input),
    };

    if out_path == "-" {
        std::io::stdout()
            .write_all(svg.as_bytes())
            .map_err(|e| e.to_string())?;
    } else {
        std::fs::write(&out_path, &svg).map_err(|e| e.to_string())?;
        eprintln!("{input} → {out_path} ({} bytes)", svg.len());
    }
    Ok(())
}

fn default_output(input: &str) -> String {
    let p = Path::new(input);
    let mut out = PathBuf::from(p);
    out.set_extension("svg");
    out.to_string_lossy().into_owned()
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}\n\n{USAGE}");
    ExitCode::FAILURE
}
