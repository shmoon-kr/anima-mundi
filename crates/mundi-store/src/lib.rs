//! State in SQLite: accounts, characters, inventories and equipment, the world's changing state, and the
//! append-only event log (D10). Content is not here: it is files.
//!
//! So far: accounts (a name and an Argon2id hash; the password itself is never stored or logged,
//! PROTOCOL.md §5) and where each character is.

use std::path::Path;

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Hash(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Db(e) => write!(f, "database: {e}"),
            Error::Hash(e) => write!(f, "password hash: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub enum Login {
    /// The account exists and the password is right.
    Ok,
    /// No account by that name: created with this password.
    Created,
    WrongPassword,
}

pub struct Store {
    db: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<Store> {
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS accounts (
                 name TEXT PRIMARY KEY,      -- lower case
                 display TEXT NOT NULL,      -- as first typed, capitalised
                 password_hash TEXT NOT NULL,
                 created INTEGER NOT NULL DEFAULT (unixepoch())
             );
             CREATE TABLE IF NOT EXISTS characters (
                 name TEXT PRIMARY KEY REFERENCES accounts(name),
                 room TEXT
             );",
        )?;
        Ok(Store { db })
    }

    /// Logs in, creating the account the first time a name is used (tbaMUD asks to confirm a new
    /// name; a client that wants that asks before sending).
    pub fn login(&self, name: &str, password: &str) -> Result<Login> {
        let key = name.to_lowercase();
        let hash: Option<String> =
            self.db.query_row("SELECT password_hash FROM accounts WHERE name = ?1", [&key], |r| r.get(0)).optional()?;
        match hash {
            Some(h) => {
                let parsed = PasswordHash::new(&h).map_err(|e| Error::Hash(e.to_string()))?;
                Ok(if Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok() {
                    Login::Ok
                } else {
                    Login::WrongPassword
                })
            }
            None => {
                let hash = Argon2::default().hash_password(password.as_bytes()).map_err(|e| Error::Hash(e.to_string()))?;
                self.db.execute(
                    "INSERT INTO accounts (name, display, password_hash) VALUES (?1, ?2, ?3)",
                    params![key, display_name(name), hash.to_string()],
                )?;
                self.db.execute("INSERT INTO characters (name, room) VALUES (?1, NULL)", [&key])?;
                Ok(Login::Created)
            }
        }
    }

    /// The name as the world shows it.
    pub fn display(&self, name: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT display FROM accounts WHERE name = ?1", [name.to_lowercase()], |r| r.get(0)).optional()?)
    }

    pub fn room(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row("SELECT room FROM characters WHERE name = ?1", [name.to_lowercase()], |r| r.get::<_, Option<String>>(0))
            .optional()?
            .flatten())
    }

    pub fn save_rooms<'a>(&mut self, rooms: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<()> {
        let tx = self.db.transaction()?;
        for (name, room) in rooms {
            tx.execute("UPDATE characters SET room = ?2 WHERE name = ?1", params![name.to_lowercase(), room])?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// tbaMUD's rule for names: letters only (interpreter.c _parse_name), 2 to 20 of them.
pub fn valid_name(name: &str) -> bool {
    (2..=20).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphabetic())
}

/// "aNA" -> "Ana" (tbaMUD capitalises a new name).
pub fn display_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut c = lower.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_and_rooms_survive_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mundi.db");
        {
            let mut s = Store::open(&path).unwrap();
            assert_eq!(s.login("aNA", "pw1").unwrap(), Login::Created);
            assert_eq!(s.login("ana", "pw1").unwrap(), Login::Ok);
            assert_eq!(s.login("Ana", "nope").unwrap(), Login::WrongPassword);
            assert_eq!(s.room("Ana").unwrap(), None);
            s.save_rooms([("Ana", "tba:30:room:3002")]).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.display("ana").unwrap().as_deref(), Some("Ana"));
        assert_eq!(s.room("ANA").unwrap().as_deref(), Some("tba:30:room:3002"));
        let stored: String = s.db.query_row("SELECT password_hash FROM accounts", [], |r| r.get(0)).unwrap();
        assert!(stored.starts_with("$argon2id$") && !stored.contains("pw1"));
    }

    #[test]
    fn names() {
        assert!(valid_name("Ana") && !valid_name("A") && !valid_name("Ana1") && !valid_name("아나"));
        assert_eq!(display_name("vALLEN"), "Vallen");
    }
}
