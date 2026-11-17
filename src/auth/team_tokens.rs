use std::{
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// How often the server re-reads `TURBO_TOKENS_DIR` by default.
pub const DEFAULT_RELOAD_INTERVAL: Duration = Duration::from_secs(10);

/// A SHA-256 digest of one team's token.
type TokenHash = [u8; 32];

/// What a presented token proves about a request.
#[derive(Debug, PartialEq, Eq)]
pub enum TeamCheck {
    /// The token belongs to the team the request names.
    Valid,
    /// The token belongs to another team, or the request names no team.
    OtherTeam,
    /// The token belongs to no team.
    UnknownToken,
}

/// Per-team tokens read from `TURBO_TOKENS_DIR`.
///
/// The directory holds one file per team: the file name is the team (the
/// `TURBO_TEAM` value Turborepo sends as `slug` or `teamId`) and the content
/// is the lowercase hex SHA-256 of that team's token. The server only ever
/// holds hashes, so the directory can be shared without the tokens.
pub struct TeamTokens {
    dir: PathBuf,
    hashes: RwLock<Arc<HashMap<String, TokenHash>>>,
}

impl fmt::Debug for TeamTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Team names only; the hashes stay out of Debug output.
        f.debug_struct("TeamTokens")
            .field("dir", &self.dir)
            .field("teams", &self.current().len())
            .finish()
    }
}

impl TeamTokens {
    /// Reads the directory once. An unreadable directory is an error, so the
    /// server refuses to start rather than reject every request.
    pub fn load(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        let hashes = read_dir(&dir)?;
        tracing::info!(teams = hashes.len(), "Loaded per-team tokens");
        Ok(Self {
            dir,
            hashes: RwLock::new(Arc::new(hashes)),
        })
    }

    /// Re-reads the directory. When it cannot be read, the last good set stays
    /// in use: a transient failure must not lock every team out.
    pub fn reload(&self) {
        match read_dir(&self.dir) {
            Ok(hashes) => {
                if *self.current() != hashes {
                    tracing::info!(teams = hashes.len(), "Reloaded per-team tokens");
                    *self.hashes.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(hashes);
                }
            }
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "Could not re-read TURBO_TOKENS_DIR. Keeping the last good tokens"
                );
            }
        }
    }

    /// Re-reads the directory every `interval` for as long as the runtime
    /// runs. Kubernetes updates a mounted Secret by swapping a symlink, which
    /// a poll sees without any file watcher.
    pub fn watch(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // The first tick fires at once; `load` has just read the directory.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let tokens = Arc::clone(&self);
                if let Err(error) = tokio::task::spawn_blocking(move || tokens.reload()).await {
                    tracing::error!(error = %error, "Per-team token reload failed");
                }
            }
        });
    }

    /// Checks a token against the team a request names.
    ///
    /// Every stored hash is compared in constant time, so the answer does not
    /// depend on how many bytes of a guess were right.
    pub fn check(&self, token: &str, team: Option<&str>) -> TeamCheck {
        let presented: TokenHash = Sha256::digest(token.as_bytes()).into();
        let hashes = self.current();

        let mut known = false;
        let mut for_team = false;
        for (name, expected) in hashes.iter() {
            if bool::from(presented.ct_eq(expected)) {
                known = true;
                for_team |= team == Some(name.as_str());
            }
        }

        match (for_team, known) {
            (true, _) => TeamCheck::Valid,
            (false, true) => TeamCheck::OtherTeam,
            (false, false) => TeamCheck::UnknownToken,
        }
    }

    fn current(&self) -> Arc<HashMap<String, TokenHash>> {
        Arc::clone(&self.hashes.read().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Reads every team file in `dir`. Names starting with `.` are skipped: they
/// are Kubernetes' `..data` links and timestamped directories, or editor files.
/// A file whose content is not a SHA-256 hex digest is skipped with a warning.
fn read_dir(dir: &Path) -> io::Result<HashMap<String, TokenHash>> {
    let mut hashes = HashMap::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Some(team) = entry.file_name().to_str().map(str::to_owned) else {
            tracing::warn!("Skipping a TURBO_TOKENS_DIR entry whose name is not UTF-8");
            continue;
        };
        if team.starts_with('.') {
            continue;
        }

        // Follows symlinks: a mounted Secret's keys link into `..data/`.
        let path = entry.path();
        if !fs::metadata(&path)?.is_file() {
            continue;
        }

        match parse_sha256_hex(&fs::read_to_string(&path)?) {
            Some(hash) => {
                hashes.insert(team, hash);
            }
            None => {
                tracing::warn!(
                    team = %team,
                    "Skipping a team whose token file is not a SHA-256 hex digest"
                );
            }
        }
    }
    Ok(hashes)
}

/// Parses 64 hex characters, ignoring trailing whitespace such as a newline.
fn parse_sha256_hex(content: &str) -> Option<TokenHash> {
    let hex = content.trim_end().as_bytes();
    if hex.len() != 64 {
        return None;
    }

    let mut hash = [0u8; 32];
    let (pairs, _) = hex.as_chunks::<2>();
    for (byte, [high, low]) in hash.iter_mut().zip(pairs) {
        let high = char::from(*high).to_digit(16)?;
        let low = char::from(*low).to_digit(16)?;
        *byte = u8::try_from(high << 4 | low).ok()?;
    }
    Some(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// An arbitrary SHA-256 hex digest.
    const ALPHA_HASH: &str = "8f0f7ba4b0e3e0e0c6a4c0fd6ab1a7a5fb1d1f7d2f3c1a0b8e2f1c0d9e8f7a6b";

    fn hash_hex(token: &str) -> String {
        Sha256::digest(token.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("decay-team-tokens-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tokens_for(name: &str, teams: &[(&str, &str)]) -> (PathBuf, TeamTokens) {
        let dir = temp_dir(name);
        for (team, token) in teams {
            fs::write(dir.join(team), format!("{}\n", hash_hex(token))).unwrap();
        }
        let tokens = TeamTokens::load(&dir).unwrap();
        (dir, tokens)
    }

    #[test]
    fn parses_a_lowercase_digest_with_a_trailing_newline() {
        let hash = parse_sha256_hex(&format!("{ALPHA_HASH}\n")).unwrap();
        assert_eq!(hash[0], 0x8f);
        assert_eq!(hash[31], 0x6b);
    }

    #[test]
    fn rejects_content_that_is_not_a_digest() {
        assert_eq!(parse_sha256_hex(""), None);
        assert_eq!(parse_sha256_hex("abc"), None);
        assert_eq!(parse_sha256_hex(&ALPHA_HASH[..63]), None);
        assert_eq!(parse_sha256_hex(&format!("{ALPHA_HASH}00")), None);
        assert_eq!(parse_sha256_hex(&ALPHA_HASH.replace('8', "g")), None);
        assert_eq!(parse_sha256_hex(&format!(" {}", &ALPHA_HASH[1..])), None);
    }

    #[test]
    fn accepts_the_token_of_the_named_team() {
        let (_, tokens) = tokens_for("valid", &[("alpha", "alpha-token")]);
        assert_eq!(tokens.check("alpha-token", Some("alpha")), TeamCheck::Valid);
    }

    #[test]
    fn rejects_the_token_of_another_team() {
        let (_, tokens) = tokens_for("other", &[("alpha", "alpha-token"), ("beta", "beta-token")]);
        assert_eq!(
            tokens.check("beta-token", Some("alpha")),
            TeamCheck::OtherTeam
        );
    }

    #[test]
    fn rejects_a_known_token_when_no_team_is_named() {
        let (_, tokens) = tokens_for("no-team", &[("alpha", "alpha-token")]);
        assert_eq!(tokens.check("alpha-token", None), TeamCheck::OtherTeam);
    }

    #[test]
    fn rejects_an_unknown_token() {
        let (_, tokens) = tokens_for("unknown", &[("alpha", "alpha-token")]);
        assert_eq!(
            tokens.check("guess", Some("alpha")),
            TeamCheck::UnknownToken
        );
        assert_eq!(tokens.check("guess", None), TeamCheck::UnknownToken);
    }

    #[test]
    fn rejects_a_team_that_has_no_file() {
        let (_, tokens) = tokens_for("missing-team", &[("alpha", "alpha-token")]);
        assert_eq!(
            tokens.check("alpha-token", Some("gamma")),
            TeamCheck::OtherTeam
        );
    }

    #[test]
    fn accepts_teams_that_share_a_token() {
        let (_, tokens) = tokens_for("shared", &[("alpha", "same-token"), ("beta", "same-token")]);
        assert_eq!(tokens.check("same-token", Some("alpha")), TeamCheck::Valid);
        assert_eq!(tokens.check("same-token", Some("beta")), TeamCheck::Valid);
    }

    #[test]
    fn skips_dotfiles_directories_and_malformed_files() {
        let dir = temp_dir("skips");
        fs::write(dir.join(".hidden"), hash_hex("hidden-token")).unwrap();
        fs::create_dir(dir.join("..2026_11_14")).unwrap();
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(dir.join("broken"), "not a digest").unwrap();
        fs::write(dir.join("alpha"), hash_hex("alpha-token")).unwrap();

        let tokens = TeamTokens::load(&dir).unwrap();

        assert_eq!(tokens.current().len(), 1);
        assert_eq!(
            tokens.check("hidden-token", Some(".hidden")),
            TeamCheck::UnknownToken
        );
        assert_eq!(tokens.check("alpha-token", Some("alpha")), TeamCheck::Valid);
    }

    /// Kubernetes mounts a Secret as `..data -> ..<timestamp>/` with one
    /// `<key> -> ..data/<key>` link per key, and swaps `..data` on update.
    #[cfg(unix)]
    #[test]
    fn reads_a_mounted_secret_through_its_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = temp_dir("secret-mount");
        let first = dir.join("..2026_11_14_first");
        fs::create_dir(&first).unwrap();
        fs::write(first.join("alpha"), hash_hex("alpha-token")).unwrap();
        symlink("..2026_11_14_first", dir.join("..data")).unwrap();
        symlink("..data/alpha", dir.join("alpha")).unwrap();

        let tokens = TeamTokens::load(&dir).unwrap();
        assert_eq!(tokens.check("alpha-token", Some("alpha")), TeamCheck::Valid);

        let second = dir.join("..2026_11_14_second");
        fs::create_dir(&second).unwrap();
        fs::write(second.join("alpha"), hash_hex("rotated-token")).unwrap();
        symlink("..2026_11_14_second", dir.join("..data_tmp")).unwrap();
        fs::rename(dir.join("..data_tmp"), dir.join("..data")).unwrap();
        tokens.reload();

        assert_eq!(
            tokens.check("alpha-token", Some("alpha")),
            TeamCheck::UnknownToken
        );
        assert_eq!(
            tokens.check("rotated-token", Some("alpha")),
            TeamCheck::Valid
        );
    }

    #[test]
    fn picks_up_new_and_removed_teams_on_reload() {
        let (dir, tokens) = tokens_for("reload", &[("alpha", "alpha-token")]);

        fs::write(dir.join("beta"), hash_hex("beta-token")).unwrap();
        fs::remove_file(dir.join("alpha")).unwrap();
        tokens.reload();

        assert_eq!(tokens.check("beta-token", Some("beta")), TeamCheck::Valid);
        assert_eq!(
            tokens.check("alpha-token", Some("alpha")),
            TeamCheck::UnknownToken
        );
    }

    #[test]
    fn keeps_the_last_good_tokens_when_the_directory_goes_away() {
        let (dir, tokens) = tokens_for("gone", &[("alpha", "alpha-token")]);

        fs::remove_dir_all(&dir).unwrap();
        tokens.reload();

        assert_eq!(tokens.check("alpha-token", Some("alpha")), TeamCheck::Valid);
    }

    #[test]
    fn fails_to_load_a_missing_directory() {
        let dir = temp_dir("missing").join("does-not-exist");
        assert!(TeamTokens::load(dir).is_err());
    }

    #[test]
    fn keeps_hashes_out_of_debug_output() {
        let (_, tokens) = tokens_for("debug", &[("alpha", "alpha-token")]);
        let debug = format!("{tokens:?}");
        assert!(debug.contains("teams: 1"), "{debug}");
        assert!(!debug.contains(&hash_hex("alpha-token")), "{debug}");
    }
}
