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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Classe, Student};

    #[test]
    fn enregistrer_puis_recharger_restitue_les_donnees() {
        let chemin = std::env::temp_dir().join(format!("st-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&chemin);
        let store = Store::new(chemin.clone());

        store
            .modify(|d| {
                d.classes.push(Classe {
                    id: "abc".into(),
                    nom: "6e A".into(),
                    students: vec![Student { id: "e1".into(), prenom: "Jean".into(), nom: "Dupont".into(), photo: None }],
                });
                Ok(())
            })
            .expect("modification");

        let recharge = Store::new(chemin.clone());
        let classes = recharge.read(|d| Ok(d.classes.clone())).unwrap();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].students[0].prenom, "Jean");
        std::fs::remove_file(chemin).ok();
    }

    #[test]
    fn classe_ergonomie_matches_name() {
        let e = Student { id: "1".into(), prenom: "Jean".into(), nom: "Dupont".into(), photo: None };
        assert!(e.matches_name("jean", "DUPONT"));
        assert!(!e.matches_name("Jeann", "Dupont"));
    }
}
