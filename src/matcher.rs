use std::collections::HashSet;
use std::{env, fs};

use crate::store::Scores;

/// Optimal string alignment distance: Levenshtein plus adjacent transposition,
/// so `sduo` -> `sudo` costs 1.
pub fn osa(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, v) in d[0].iter_mut().enumerate() {
        *v = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[n][m]
}

/// Short words get a tight budget, otherwise `ls` would match half of $PATH.
pub fn max_distance(typo: &str) -> usize {
    match typo.chars().count() {
        0..=3 => 1,
        4..=6 => 2,
        _ => 3,
    }
}

/// Every executable name on $PATH plus names supplied by the shell
/// (builtins, functions, abbreviations).
pub fn candidates(extra: impl IntoIterator<Item = String>) -> HashSet<String> {
    let mut out: HashSet<String> = extra.into_iter().filter(|s| !s.is_empty()).collect();
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    out.insert(name);
                }
            }
        }
    }
    out
}

/// Best correction for `typo`, or None. A learned mapping wins outright;
/// otherwise rank by edit distance, then personal score, then cheap
/// tie-breakers (same letters reordered, same first letter, same length,
/// alphabetical). Reordering beats substitution: `sl` means `ls`, not `sb`.
pub fn best(typo: &str, cands: &HashSet<String>, scores: &Scores) -> Option<String> {
    if cands.contains(typo) {
        return None;
    }
    if let Some(learned) = scores.learned(typo)
        && cands.contains(learned)
    {
        return Some(learned.to_owned());
    }
    let max = max_distance(typo);
    let tlen = typo.chars().count();
    let first = typo.chars().next();
    cands
        .iter()
        .filter(|c| c.chars().count().abs_diff(tlen) <= max)
        .filter(|c| !scores.banned(typo, c))
        .filter_map(|c| {
            let d = osa(typo, c);
            (d <= max).then(|| {
                (
                    d,
                    -scores.get(typo, c),
                    !same_letters(typo, c),
                    c.chars().next() != first,
                    c.chars().count().abs_diff(tlen),
                    c.clone(),
                )
            })
        })
        .min()
        .map(|k| k.5)
}

fn same_letters(a: &str, b: &str) -> bool {
    let mut a: Vec<char> = a.chars().collect();
    let mut b: Vec<char> = b.chars().collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn osa_counts_transposition_as_one() {
        assert_eq!(osa("sduo", "sudo"), 1);
        assert_eq!(osa("sl", "ls"), 1);
        assert_eq!(osa("gti", "git"), 1);
        assert_eq!(osa("kitten", "sitting"), 3);
        assert_eq!(osa("", "abc"), 3);
    }

    #[test]
    fn picks_closest() {
        let c = set(&["sudo", "sed", "su", "ls", "sb", "git", "grep"]);
        let s = Scores::default();
        assert_eq!(best("sduo", &c, &s).as_deref(), Some("sudo"));
        assert_eq!(best("gti", &c, &s).as_deref(), Some("git"));
        assert_eq!(best("gerp", &c, &s).as_deref(), Some("grep"));
        assert_eq!(best("sl", &c, &s).as_deref(), Some("ls"));
    }

    #[test]
    fn no_match_beyond_budget() {
        let c = set(&["sudo"]);
        assert_eq!(best("xyz", &c, &Scores::default()), None);
    }

    #[test]
    fn existing_command_is_not_corrected() {
        let c = set(&["sl", "ls"]);
        assert_eq!(best("sl", &c, &Scores::default()), None);
    }
}
