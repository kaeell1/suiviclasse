use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::model::Data;

/// Stockage applicatif : toutes les données dans un fichier JSON local,
/// rechargé en mémoire et réécrit (écriture atomique) à chaque changement.
pub struct Store {
    data: Mutex<Data>,
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        let data = Self::read_from_disk(&path).unwrap_or_default();
        Self {
            data: Mutex::new(data),
            path,
        }
    }

    /// Point d'entrée partagé : `Arc<Store>` dans les handlers axum.
    pub fn shared(path: PathBuf) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::new(path))
    }

    fn read_from_disk(path: &Path) -> Option<Data> {
        let bytes = std::fs::read(path).ok()?;
        match serde_json::from_slice::<Data>(&bytes) {
            Ok(data) => Some(data),
            Err(erreur) => {
                eprintln!("Impossible de relire {} : {erreur}", path.display());
                // Conserver une copie du fichier illisible avant de repartir à neuf.
                let _ = std::fs::copy(path, path.with_extension("json.corrup"));
                None
            }
        }
    }

    /// Exécute une modification sur les données, puis sauvegarde sur le disque.
    pub fn modify<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Data) -> Result<T, String>,
    {
        let mut data = self
            .data
            .lock()
            .map_err(|_| "verrou du stockage empoisonné".to_string())?;
        let resultat = f(&mut data)?;
        self.write_to_disk(&data)?;
        Ok(resultat)
    }

    /// Exécute une lecture sans modification.
    pub fn read<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Data) -> Result<T, String>,
    {
        let data = self
            .data
            .lock()
            .map_err(|_| "verrou du stockage empoisonné".to_string())?;
        f(&data)
    }

    fn write_to_disk(&self, data: &Data) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes =
            serde_json::to_vec_pretty(data).map_err(|e| format!("sérialisation : {e}"))?;
        let temporaire = self.path.with_extension("json.tmp");
        std::fs::write(&temporaire, bytes).map_err(|e| format!("écriture : {e}"))?;
        std::fs::rename(&temporaire, &self.path).map_err(|e| format!("renommage : {e}"))?;
        Ok(())
    }
}
