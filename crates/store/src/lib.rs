//! Flat-file persistence for DiveBridge — see `CONTRACT.md`.
//!
//! A [`Store`] is a thin, dependency-light layer over a data directory. It keeps
//! the full dive history as Git-friendly flat files (no DB):
//!
//! ```text
//! <root>/dives/<dive_id>.json   serialized core::Dive (pretty JSON)
//! <root>/raw/<source_id>/<file> verbatim original artifacts (immutable bytes)
//! <root>/merges.json            { dive_id: [source_id, ...] } groupings
//! <root>/rules.json             Vec<core::ClassificationRule>
//! <root>/ledger.json            { dive_id: SyncState } mirror for quick scan
//! ```
//!
//! Design notes:
//! - Dive writes are atomic (write to `<file>.tmp`, then `fs::rename`).
//! - The raw layer is immutable: an artifact is written once and never modified;
//!   re-writing identical bytes is a no-op, differing bytes is an error.
//! - Missing index files (`rules.json`, `merges.json`, `ledger.json`) read as
//!   sensible empty defaults rather than errors.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use divebridge_core as core;

use divebridge_core::hash::sha256_hex;
use divebridge_core::model::{ArtifactRef, Dive, DiveId, SourceId, SourceRecording, SyncState};
use divebridge_core::ClassificationRule;

/// Errors returned by [`Store`] operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("json error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("immutable artifact already exists with different content: {0}")]
    Immutable(PathBuf),
}

/// Convenience alias for store results.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Outcome of [`Store::upsert_source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// The source's `dedup_key` was already present; nothing changed.
    Unchanged,
    /// The source was new and was appended to the dive.
    Added,
}

/// Flat-file persistence over a data directory.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open (or create) a store rooted at `root`, creating the root and its
    /// subdirectories (`dives/`, `raw/`) if missing.
    pub fn open(root: impl AsRef<Path>) -> Result<Store> {
        let root = root.as_ref().to_path_buf();
        create_dir_all(&root)?;
        create_dir_all(&root.join("dives"))?;
        create_dir_all(&root.join("raw"))?;
        Ok(Store { root })
    }

    /// The data directory backing this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- paths -------------------------------------------------------------

    fn dives_dir(&self) -> PathBuf {
        self.root.join("dives")
    }

    fn dive_path(&self, id: &DiveId) -> PathBuf {
        self.dives_dir().join(format!("{}.json", id.0))
    }

    fn raw_dir(&self, source_id: &SourceId) -> PathBuf {
        self.root.join("raw").join(&source_id.0)
    }

    fn merges_path(&self) -> PathBuf {
        self.root.join("merges.json")
    }

    fn rules_path(&self) -> PathBuf {
        self.root.join("rules.json")
    }

    fn ledger_path(&self) -> PathBuf {
        self.root.join("ledger.json")
    }

    // --- dives -------------------------------------------------------------

    /// List the ids of all persisted dives (order is unspecified).
    pub fn list_dive_ids(&self) -> Result<Vec<DiveId>> {
        let dir = self.dives_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::io(&dir, e)),
        };

        let mut ids = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(&dir, e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(DiveId(stem.to_string()));
                }
            }
        }
        Ok(ids)
    }

    /// Load a single dive by id.
    pub fn load_dive(&self, id: &DiveId) -> Result<Dive> {
        let path = self.dive_path(id);
        match read_json(&path)? {
            Some(dive) => Ok(dive),
            None => Err(StoreError::NotFound(format!("dive {}", id.0))),
        }
    }

    /// Persist a dive atomically, then mirror its [`SyncState`] into the ledger.
    pub fn save_dive(&self, dive: &Dive) -> Result<()> {
        write_json_atomic(&self.dive_path(&dive.id), dive)?;
        self.update_ledger(&dive.id, &dive.sync)?;
        Ok(())
    }

    /// Delete a dive (and its ledger entry). Missing dive => [`StoreError::NotFound`].
    pub fn delete_dive(&self, id: &DiveId) -> Result<()> {
        let path = self.dive_path(id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::NotFound(format!("dive {}", id.0)));
            }
            Err(e) => return Err(StoreError::io(&path, e)),
        }

        let mut ledger = self.load_ledger()?;
        if ledger.remove(&id.0).is_some() {
            write_json_atomic(&self.ledger_path(), &ledger)?;
        }
        Ok(())
    }

    /// Append a source recording to a dive, deduplicating by
    /// [`SourceRecording::dedup_key`]. Idempotent: re-importing a source already
    /// present is a no-op ([`UpsertOutcome::Unchanged`]).
    pub fn upsert_source(
        &self,
        dive_id: &DiveId,
        source: SourceRecording,
    ) -> Result<UpsertOutcome> {
        let mut dive = self.load_dive(dive_id)?;
        let key = source.dedup_key();
        if dive.sources.iter().any(|s| s.dedup_key() == key) {
            return Ok(UpsertOutcome::Unchanged);
        }
        dive.sources.push(source);
        self.save_dive(&dive)?;
        Ok(UpsertOutcome::Added)
    }

    // --- raw artifacts -----------------------------------------------------

    /// Write a verbatim original artifact under `raw/<source_id>/<filename>` and
    /// return an [`ArtifactRef`] (store-relative path, sha256, byte length).
    ///
    /// Raw artifacts are immutable: re-writing identical bytes is a no-op, but
    /// writing different bytes to an existing path is refused
    /// ([`StoreError::Immutable`]).
    pub fn save_raw_artifact(
        &self,
        source_id: &SourceId,
        filename: &str,
        bytes: &[u8],
    ) -> Result<ArtifactRef> {
        let dir = self.raw_dir(source_id);
        let path = dir.join(filename);

        match fs::read(&path) {
            Ok(existing) => {
                if existing != bytes {
                    return Err(StoreError::Immutable(path));
                }
                // Identical content already on disk: idempotent no-op.
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                create_dir_all(&dir)?;
                fs::write(&path, bytes).map_err(|e| StoreError::io(&path, e))?;
            }
            Err(e) => return Err(StoreError::io(&path, e)),
        }

        Ok(ArtifactRef {
            path: format!("raw/{}/{}", source_id.0, filename),
            sha256: sha256_hex(bytes),
            bytes: bytes.len() as u64,
        })
    }

    // --- rules -------------------------------------------------------------

    /// Load the classification rules. Missing `rules.json` => empty vec.
    pub fn load_rules(&self) -> Result<Vec<ClassificationRule>> {
        Ok(read_json(&self.rules_path())?.unwrap_or_default())
    }

    /// Persist the classification rules atomically.
    pub fn save_rules(&self, rules: &[ClassificationRule]) -> Result<()> {
        write_json_atomic(&self.rules_path(), &rules)
    }

    // --- merges ------------------------------------------------------------

    /// Record (or overwrite) the merge grouping for a dive: which source ids
    /// belong to it. Persisted so a device re-sync does not un-merge.
    pub fn record_merge(&self, dive_id: &DiveId, sources: &[SourceId]) -> Result<()> {
        let mut merges = self.load_merges()?;
        let ids = sources.iter().map(|s| s.0.clone()).collect();
        merges.insert(dive_id.0.clone(), ids);
        write_json_atomic(&self.merges_path(), &merges)
    }

    /// Load the persisted merge groupings. Missing `merges.json` => empty map.
    pub fn load_merges(&self) -> Result<BTreeMap<String, Vec<String>>> {
        Ok(read_json(&self.merges_path())?.unwrap_or_default())
    }

    // --- ledger ------------------------------------------------------------

    /// Load the sync ledger ({ dive_id: SyncState }). Missing file => empty map.
    fn load_ledger(&self) -> Result<BTreeMap<String, SyncState>> {
        Ok(read_json(&self.ledger_path())?.unwrap_or_default())
    }

    fn update_ledger(&self, dive_id: &DiveId, sync: &SyncState) -> Result<()> {
        let mut ledger = self.load_ledger()?;
        ledger.insert(dive_id.0.clone(), sync.clone());
        write_json_atomic(&self.ledger_path(), &ledger)
    }
}

impl StoreError {
    fn io(path: impl AsRef<Path>, source: io::Error) -> StoreError {
        StoreError::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    fn json(path: impl AsRef<Path>, source: serde_json::Error) -> StoreError {
        StoreError::Json {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

// --- free helpers ----------------------------------------------------------

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| StoreError::io(path, e))
}

/// Read and deserialize JSON, returning `Ok(None)` if the file does not exist.
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(StoreError::io(path, e)),
    };
    let value = serde_json::from_slice(&bytes).map_err(|e| StoreError::json(path, e))?;
    Ok(Some(value))
}

/// Serialize `value` as pretty JSON and write it atomically: write to a sibling
/// `<file>.tmp`, then `fs::rename` over the target.
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(value).map_err(|e| StoreError::json(path, e))?;

    let tmp = with_tmp_extension(path);
    fs::write(&tmp, &json).map_err(|e| StoreError::io(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| StoreError::io(path, e))?;
    Ok(())
}

fn with_tmp_extension(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}
