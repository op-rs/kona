//! Bootnode Store

use discv5::Enr;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

/// On-disk storage for [`Enr`]s.
///
/// The [`BootStore`] is a simple JSON file that holds
/// the list of [`Enr`]s that have been successfully
/// peered.
#[derive(Debug, serde::Serialize)]
pub struct BootStore {
    /// The file path for the [`BootStore`].
    #[serde(skip)]
    pub path: PathBuf,
    /// [`Enr`]s for peers.
    pub peers: Vec<Enr>,
}

// This custom implementation of `Deserialize` allows us to ignore
// enrs that have an invalid string format in the store.
impl<'de> serde::Deserialize<'de> for BootStore {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let peers: Vec<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
        let mut store = Self { path: PathBuf::new(), peers: Vec::new() };
        for peer in peers {
            match serde_json::from_value::<Enr>(peer) {
                Ok(enr) => {
                    store.peers.push(enr);
                }
                Err(e) => {
                    warn!("Failed to deserialize ENR: {:?}", e);
                }
            }
        }
        Ok(store)
    }
}

impl BootStore {
    /// Adds an [`Enr`] to the store.
    ///
    /// This method will **note** panic on failure to write to disk.
    /// Instead, it is the responsibility of the caller to ensure the
    /// store is written to disk by calling [`BootStore::sync`] prior
    /// to dropping the store.
    pub fn add_enr(&mut self, enr: Enr) {
        if self.peers.contains(&enr) {
            return;
        }
        debug!(target: "bootstore", "Adding enr to the boot store: {}", enr);
        self.peers.push(enr);
        if let Err(e) = self.write_to_file() {
            warn!(target: "bootstore", "Failed to write boot store to disk: {:?}", e);
        }
    }

    /// Returns the number of peers in the bootstore that
    /// have the [`crate::OpStackEnr::OP_CL_KEY`] in the ENR.
    pub fn valid_peers(&self) -> Vec<&Enr> {
        self.peers
            .iter()
            .filter(|enr| enr.get_raw_rlp(crate::OpStackEnr::OP_CL_KEY.as_bytes()).is_some())
            .collect()
    }

    /// Returns the number of peers that contain the
    /// [`crate::OpStackEnr::OP_CL_KEY`] in the ENR *and*
    /// have the correct chain id and version.
    pub fn valid_peers_with_chain_id(&self, chain_id: u64) -> Vec<&Enr> {
        self.peers
            .iter()
            .filter(|enr| crate::EnrValidation::validate(enr, chain_id).is_valid())
            .collect()
    }

    /// Returns the number of peers in the in-memory store.
    pub const fn len(&self) -> usize {
        self.peers.len()
    }

    /// Returns if the in-memory store is empty.
    pub const fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Returns the list of peer [`Enr`]s.
    ///
    /// This method will **note** panic on failure to read from disk.
    pub fn peers(&mut self) -> &Vec<Enr> {
        let store = Self::from_file(&self.path);
        self.merge(store.peers);
        &self.peers
    }

    /// Merges the given list of [`Enr`]s with the current list of peers.
    pub fn merge(&mut self, peers: Vec<Enr>) {
        for peer in peers {
            if !self.peers.contains(&peer) {
                self.peers.push(peer);
            }
        }
    }

    /// Syncs the [`BootStore`] with the contents on disk.
    pub fn sync(&mut self) {
        let _ = self.peers();
        if let Err(e) = self.write_to_file() {
            warn!(target: "bootstore", "Failed to write boot store to disk: {:?}", e);
        }
    }

    /// Writes the store to disk.
    fn write_to_file(&mut self) -> Result<(), std::io::Error> {
        // If the directory does not exist, create it.
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(&self.path)?;
        serde_json::to_writer(file, &self.peers)?;
        Ok(())
    }

    /// Returns all available bootstores for the given data directory.
    pub fn available(datadir: Option<PathBuf>) -> Vec<u64> {
        let mut bootstores = Vec::new();
        let path = datadir.unwrap_or_else(|| {
            let mut home = dirs::home_dir().expect("Failed to get home directory");
            home.push(".kona");
            home
        });
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(chain_id) = entry.file_name().to_string_lossy().parse::<u64>() {
                    bootstores.push(chain_id);
                }
            }
        }
        bootstores
    }

    /// Returns the [`PathBuf`] for the given chain id.
    pub fn path(chain_id: u64, datadir: Option<PathBuf>) -> PathBuf {
        let mut path = datadir.unwrap_or_else(|| {
            let mut home = dirs::home_dir().expect("Failed to get home directory");
            home.push(".kona");
            home
        });
        path.push(chain_id.to_string());
        path.push("bootstore.json");
        path
    }

    /// Reads a new [`BootStore`] from the given chain id and data directory.
    ///
    /// If the file cannot be read, an empty [`BootStore`] is returned.
    pub fn from_chain_id(chain_id: u64, datadir: Option<PathBuf>, bootnodes: Vec<Enr>) -> Self {
        let path = Self::path(chain_id, datadir);
        let mut store = Self::from_file(&path);

        // Add the bootnodes to the bootstore.
        store.merge(bootnodes);

        store
    }

    /// Reads a new [`BootStore`] from the given chain id and data directory.
    ///
    /// If the file cannot be read, an empty [`BootStore`] is returned.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let p = path.as_ref().to_path_buf();
        let peers = File::open(path)
            .map(|file| {
                let reader = BufReader::new(file);
                debug!(target: "bootstore", "Reading boot store from disk: {:?}", p);
                match serde_json::from_reader(reader).map(|s: Self| s.peers) {
                    Ok(peers) => peers,
                    Err(e) => {
                        warn!(target: "bootstore", "Failed to read boot store from disk: {:?}", e);
                        Vec::new()
                    }
                }
            })
            .unwrap_or_default();
        Self { path: p, peers }
    }
}
