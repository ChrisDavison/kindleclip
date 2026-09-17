use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// User curation state, persisted as a single JSON file. Clippings are
/// keyed by their stable content-derived id, so the state survives
/// re-importing a newer copy of the same source file.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistentState {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Source files uploaded through the web UI, relative to the state
    /// file's directory. Reloaded on startup.
    #[serde(default)]
    pub uploads: Vec<String>,
    #[serde(default)]
    pub books: HashMap<String, BookState>,
}

impl Default for PersistentState {
    fn default() -> Self {
        PersistentState {
            version: default_version(),
            uploads: Vec::new(),
            books: HashMap::new(),
        }
    }
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookState {
    #[serde(default)]
    pub clusters: Vec<ClusterState>,
    #[serde(default)]
    pub clippings: HashMap<String, ClippingState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub clipping_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClippingState {
    #[serde(default)]
    pub marked: bool,
    #[serde(default)]
    pub ignored: bool,
    #[serde(default)]
    pub annotation: String,
}

impl PersistentState {
    pub fn load(path: &Path) -> Result<PersistentState> {
        if !path.exists() {
            return Ok(PersistentState::default());
        }
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read state file {}", path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("Invalid state file {}", path.display()))
    }

    /// Save atomically: write to a temporary file, then rename over the
    /// destination.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("Failed to create {}", dir.display()))?;
            }
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("Failed to write {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("Failed to move state into place at {}", path.display()))
    }

    pub fn book(&mut self, book_id: &str) -> &mut BookState {
        self.books.entry(book_id.to_string()).or_default()
    }
}

impl BookState {
    pub fn clipping(&mut self, clipping_id: &str) -> &mut ClippingState {
        self.clippings
            .entry(clipping_id.to_string())
            .or_default()
    }

    pub fn clipping_state(&self, clipping_id: &str) -> Option<&ClippingState> {
        self.clippings.get(clipping_id)
    }

    pub fn is_ignored(&self, clipping_id: &str) -> bool {
        self.clipping_state(clipping_id).map(|s| s.ignored).unwrap_or(false)
    }

    pub fn is_marked(&self, clipping_id: &str) -> bool {
        self.clipping_state(clipping_id).map(|s| s.marked).unwrap_or(false)
    }

    /// The id of the first cluster containing this clipping, if any.
    pub fn cluster_of(&self, clipping_id: &str) -> Option<&str> {
        self.clusters
            .iter()
            .find(|c| c.clipping_ids.iter().any(|id| id == clipping_id))
            .map(|c| c.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let mut state = PersistentState::default();
        let bs = state.book("b1");
        bs.clipping("c1").marked = true;
        bs.clipping("c2").ignored = true;
        bs.clipping("c3").annotation = "hello".into();
        bs.clusters.push(ClusterState {
            id: "cl1".into(),
            name: "Favourites".into(),
            clipping_ids: vec!["c3".into(), "c1".into()],
        });

        let dir = std::env::temp_dir().join(format!(
            "kindleclip-state-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");

        state.save(&path).unwrap();
        let loaded = PersistentState::load(&path).unwrap();
        let bs = loaded.books.get("b1").unwrap();
        assert!(bs.clipping_state("c1").unwrap().marked);
        assert!(bs.clipping_state("c2").unwrap().ignored);
        assert_eq!(bs.clipping_state("c3").unwrap().annotation, "hello");
        assert_eq!(bs.clusters[0].name, "Favourites");
        assert_eq!(bs.cluster_of("c1"), Some("cl1"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_loads_default() {
        let state = PersistentState::load(Path::new("/nonexistent/kindleclip.json")).unwrap();
        assert!(state.books.is_empty());
        assert_eq!(state.version, 1);
    }
}
