//! Debug CLI over the config backend. Phase 3 replaces this with the egui
//! GUI; until then it is the end-to-end verification tool.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use umbriel_config::config::{discovery, document::ConfigDocument, validate};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut config: Option<PathBuf> = None;
    let mut command = args
        .next()
        .context("missing command; expected path, get, or set")?;
    while command == "--config" {
        config = Some(PathBuf::from(args.next().context("--config needs a path")?));
        command = args
            .next()
            .context("missing command; expected path, get, or set")?;
    }
    let path = config.unwrap_or_else(discovery::resolve_process);

    match command.as_str() {
        "path" => {
            println!("{}", path.display());
            Ok(())
        }
        "get" => {
            let key = args
                .next()
                .context("get needs a key like general.xwayland")?;
            let doc = ConfigDocument::load(&path)
                .with_context(|| format!("loading {}", path.display()))?;
            let parts: Vec<&str> = key.split('.').collect();
            if let Some(value) = doc.get_bool(&parts) {
                println!("{value}");
            } else if let Some(value) = doc.get_integer(&parts) {
                println!("{value}");
            } else if let Some(value) = doc.get_float(&parts) {
                println!("{value}");
            } else if let Some(value) = doc.get_string(&parts) {
                println!("{value}");
            } else {
                println!("<unset>");
            }
            Ok(())
        }
        "set" => {
            let key = args
                .next()
                .context("set needs a key like general.xwayland")?;
            let value = args.next().context("set needs a value")?;
            let mut doc = ConfigDocument::load(&path)
                .with_context(|| format!("loading {}", path.display()))?;
            let parts: Vec<&str> = key.split('.').collect();
            match parse_value(&value) {
                Parsed::Bool(v) => doc.set_bool(&parts, v),
                Parsed::Integer(v) => doc.set_integer(&parts, v),
                Parsed::Float(v) => doc.set_float(&parts, v),
                Parsed::Str(v) => doc.set_string(&parts, &v),
            }
            doc.save(&path)
                .with_context(|| format!("saving {}", path.display()))?;
            println!("saved {key} = {value}");
            match validate::validate(&path) {
                Ok(report) => {
                    for diagnostic in &report.diagnostics {
                        let label = if diagnostic.is_error() {
                            "error"
                        } else {
                            "warning"
                        };
                        eprintln!("{label}: {}", diagnostic.message());
                    }
                    if report.is_ok() {
                        Ok(())
                    } else {
                        bail!("umbriel rejected the config")
                    }
                }
                Err(err) => {
                    eprintln!("note: skipped validation ({err})");
                    Ok(())
                }
            }
        }
        other => bail!("unknown command '{other}'; expected path, get, or set"),
    }
}

/// Best-effort typed parse: bool, then integer, then float, else string.
enum Parsed {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Str(String),
}

fn parse_value(raw: &str) -> Parsed {
    match raw {
        "true" => return Parsed::Bool(true),
        "false" => return Parsed::Bool(false),
        _ => {}
    }
    if let Ok(value) = raw.parse::<i64>() {
        return Parsed::Integer(value);
    }
    if let Ok(value) = raw.parse::<f64>() {
        return Parsed::Float(value);
    }
    Parsed::Str(raw.to_owned())
}
