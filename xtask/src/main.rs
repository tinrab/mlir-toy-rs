mod no_slop;

use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("no-slop") => no_slop::run(&args.collect::<Vec<_>>()),
        _ => ExitCode::from(2),
    }
}
