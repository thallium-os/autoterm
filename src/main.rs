mod matcher;
mod safety;
mod store;

use std::io::{self, BufRead, IsTerminal};
use std::process::ExitCode;

use anyhow::{Result, bail};

use store::{Event, Scores, Store};

const USAGE: &str = "usage:
  autoterm suggest [--stdin] <word>   print `<silent|confirm>\\t<fix>`, exit 1 if none
                                      --stdin: extra command names, one per line
  autoterm record <accept|reject|override> <typo> <fix>
  autoterm log [N]                    last N audit entries (default 20)
  autoterm verify                     check log checksum
  autoterm reset [--yes]              delete the log and everything learned";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("autoterm: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let store = Store::open()?;
    match args.as_slice() {
        ["suggest", rest @ ..] => {
            let (stdin, typo) = match rest {
                ["--stdin", w] => (true, *w),
                [w] => (false, *w),
                _ => bail!("{USAGE}"),
            };
            let extra: Vec<String> = if stdin && !io::stdin().is_terminal() {
                io::stdin().lock().lines().map_while(Result::ok).collect()
            } else {
                Vec::new()
            };
            // A tampered log must not steer suggestions; fall back to base ranking.
            let scores = store.scores().unwrap_or_else(|e| {
                eprintln!("autoterm: {e:#}");
                Scores::default()
            });
            let Some(fix) = matcher::best(typo, &matcher::candidates(extra), &scores) else {
                return Ok(ExitCode::FAILURE);
            };
            // Not logged: a password typed at the prompt by mistake would
            // land here. Only answers the user gave are recorded.
            println!("{}\t{fix}", safety::classify(&fix).as_str());
        }
        ["record", ev, typo, fix] => {
            let Some(ev) = Event::parse(ev).filter(|e| *e != Event::Shown) else {
                bail!("{USAGE}")
            };
            // Overrides come from "the next command that worked"; only keep
            // ones that plausibly are a fix for the typo.
            if ev == Event::Override
                && (typo == fix || matcher::osa(typo, fix) > matcher::max_distance(typo))
            {
                return Ok(ExitCode::SUCCESS);
            }
            store.append(ev, typo, fix)?;
        }
        ["log", rest @ ..] => {
            let n: usize = match rest {
                [] => 20,
                [n] => n.parse()?,
                _ => bail!("{USAGE}"),
            };
            if let Err(e) = store.verify() {
                eprintln!("autoterm: WARNING: {e:#}");
            }
            let log = std::fs::read_to_string(store.log_path()).unwrap_or_default();
            let lines: Vec<&str> = log.lines().collect();
            for l in &lines[lines.len().saturating_sub(n)..] {
                println!("{l}");
            }
        }
        ["verify"] => {
            store.verify()?;
            println!("ok");
        }
        ["reset", rest @ ..] => {
            let yes = match rest {
                [] => false,
                ["--yes"] => true,
                _ => bail!("{USAGE}"),
            };
            if !yes {
                if !io::stdin().is_terminal() {
                    bail!("refusing to reset without a terminal; pass --yes");
                }
                eprint!(
                    "autoterm: delete {} and everything learned? [y/N] ",
                    store.log_path().display()
                );
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if !matches!(answer.trim(), "y" | "Y" | "yes") {
                    eprintln!("autoterm: nothing deleted");
                    return Ok(ExitCode::FAILURE);
                }
            }
            store.reset()?;
            eprintln!("autoterm: log deleted; learning starts over");
        }
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}
