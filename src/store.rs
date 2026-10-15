//! Append-only TSV log (`ts  event  typo  correction`) that doubles as the
//! audit trail and the source of personal scores, plus a SHA-256 sidecar.
//! The checksum makes edits outside autoterm visible; it does not stop a
//! process running as the same user from rewriting both files.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// Net score at which a correction is used without ranking.
const LEARNED_AT: i64 = 2;
/// Net score at which a correction is never suggested again for that typo.
const BANNED_AT: i64 = -2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Shown,
    Accept,
    Reject,
    Override,
}

impl Event {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "shown" => Event::Shown,
            "accept" => Event::Accept,
            "reject" => Event::Reject,
            "override" => Event::Override,
            _ => return None,
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Event::Shown => "shown",
            Event::Accept => "accept",
            Event::Reject => "reject",
            Event::Override => "override",
        }
    }

    fn weight(self) -> i64 {
        match self {
            Event::Shown => 0,
            Event::Accept | Event::Override => 1,
            Event::Reject => -1,
        }
    }
}

#[derive(Default)]
pub struct Scores(HashMap<(String, String), i64>);

impl Scores {
    pub fn parse(log: &str) -> Self {
        let mut map = HashMap::new();
        for line in log.lines() {
            let mut f = line.split('\t').skip(1);
            let (Some(ev), Some(typo), Some(fix)) = (f.next(), f.next(), f.next()) else { continue };
            let Some(ev) = Event::parse(ev) else { continue };
            *map.entry((typo.to_owned(), fix.to_owned())).or_insert(0) += ev.weight();
        }
        Scores(map)
    }

    pub fn get(&self, typo: &str, fix: &str) -> i64 {
        self.0.get(&(typo.to_owned(), fix.to_owned())).copied().unwrap_or(0)
    }

    pub fn banned(&self, typo: &str, fix: &str) -> bool {
        self.get(typo, fix) <= BANNED_AT
    }

    pub fn learned(&self, typo: &str) -> Option<&str> {
        self.0
            .iter()
            .filter(|((t, _), s)| t == typo && **s >= LEARNED_AT)
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.1.cmp(&a.0.1)))
            .map(|((_, f), _)| f.as_str())
    }
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn open() -> Result<Self> {
        let base = match std::env::var_os("XDG_DATA_HOME") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join(".local/share"),
        };
        Ok(Store { dir: base.join("autoterm") })
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("log.tsv")
    }

    fn sum_path(&self) -> PathBuf {
        self.dir.join("log.tsv.sha256")
    }

    fn read_log(&self) -> Result<String> {
        match fs::read_to_string(self.log_path()) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e).context("reading log"),
        }
    }

    /// Ok(()) if the log matches its recorded checksum (or neither exists).
    pub fn verify(&self) -> Result<()> {
        let log = self.read_log()?;
        let sum = match fs::read_to_string(self.sum_path()) {
            Ok(s) => s.trim().to_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && log.is_empty() => return Ok(()),
            Err(e) => return Err(e).context("reading checksum"),
        };
        if sum != hex_sha256(log.as_bytes()) {
            bail!("{} does not match its checksum; personal data ignored", self.log_path().display());
        }
        Ok(())
    }

    pub fn scores(&self) -> Result<Scores> {
        self.verify()?;
        Ok(Scores::parse(&self.read_log()?))
    }

    /// Appends one event and rewrites the checksum. Refuses to write onto a
    /// log that already fails verification, so tampering is never laundered.
    pub fn append(&self, ev: Event, typo: &str, fix: &str) -> Result<()> {
        for s in [typo, fix] {
            if s.is_empty() || s.contains(['\t', '\n', '\r']) {
                bail!("invalid field {s:?}");
            }
        }
        fs::create_dir_all(&self.dir)?;
        let lock = File::create(self.dir.join("lock"))?;
        lock.lock()?;
        self.verify()?;
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let mut f = OpenOptions::new().create(true).append(true).open(self.log_path())?;
        writeln!(f, "{ts}\t{}\t{typo}\t{fix}", ev.as_str())?;
        f.sync_data()?;
        let sum = hex_sha256(self.read_log()?.as_bytes());
        let tmp = self.dir.join("log.tsv.sha256.tmp");
        fs::write(&tmp, format!("{sum}\n"))?;
        fs::rename(tmp, self.sum_path())?;
        Ok(())
    }
}

fn hex_sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("autoterm-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store { dir }
    }

    #[test]
    fn learns_after_two_overrides_and_bans_after_two_rejects() {
        let s = Scores::parse(
            "1\toverride\tgs\tgit\n1\toverride\tgs\tgit\n1\treject\tgs\tls\n1\treject\tgs\tls\n",
        );
        assert_eq!(s.learned("gs"), Some("git"));
        assert!(s.banned("gs", "ls"));
        assert_eq!(Scores::parse("1\toverride\tgs\tgit\n").learned("gs"), None);
    }

    #[test]
    fn append_round_trips_and_detects_tamper() {
        let st = temp_store("tamper");
        st.verify().unwrap();
        st.append(Event::Accept, "sduo", "sudo").unwrap();
        st.append(Event::Accept, "sduo", "sudo").unwrap();
        assert_eq!(st.scores().unwrap().learned("sduo"), Some("sudo"));

        fs::write(st.log_path(), "1\toverride\tls\trm\n1\toverride\tls\trm\n").unwrap();
        assert!(st.verify().is_err());
        assert!(st.append(Event::Accept, "a", "b").is_err());
        fs::remove_dir_all(&st.dir).unwrap();
    }

    #[test]
    fn rejects_tabs_in_fields() {
        let st = temp_store("tabs");
        assert!(st.append(Event::Accept, "a\tb", "c").is_err());
    }
}
