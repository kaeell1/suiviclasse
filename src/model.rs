use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Student {
    pub id: String,
    pub prenom: String,
    pub nom: String,
    #[serde(default)]
    pub photo: Option<String>,
}

impl Student {
    pub fn matches_name(&self, prenom: &str, nom: &str) -> bool {
        self.prenom.trim().eq_ignore_ascii_case(prenom.trim())
            && self.nom.trim().eq_ignore_ascii_case(nom.trim())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classe {
    pub id: String,
    pub nom: String,
    #[serde(default)]
    pub students: Vec<Student>,
}

/// Un siège : None si vide, Some(id élève) si attribué.
pub type Seat = Option<String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePlan {
    pub col: u32,
    pub row: u32,
    pub seats: Vec<Seat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Id de la classe couverte par ce plan.
    pub classe_id: String,
    #[serde(default)]
    pub nom: String,
    pub tables: Vec<TablePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub classe_id: String,
    pub eleve_id: String,
    /// Date au format AAAA-MM-JJ (lisible par l'enseignant).
    pub date: String,
    #[serde(default)]
    pub heure: String,
    /// Points : positif ou négatif.
    pub points: i32,
    pub libelle: String,
    /// Ordre de création (nanosecondes), pour trier de manière déterministe les actions à date/heure égales.
    #[serde(default)]
    pub crea: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub classe_id: String,
    pub eleve_id: String,
    #[serde(default)]
    pub texte: String,
    #[serde(default)]
    pub maj: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creneau {
    pub id: String,
    /// Index du jour (0 = lundi ... 4 = vendredi).
    pub jour: u8,
    /// Heure de début en minutes depuis minuit (ex : 480 pour 08:00).
    pub debut: u32,
    pub duree: u32,
    pub classe_id: String,
    #[serde(default)]
    pub matiere: String,
    #[serde(default)]
    pub salle: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Edt {
    #[serde(default)]
    pub creneaux: Vec<Creneau>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_actions_positives")]
    pub actions_positives: Vec<String>,
    #[serde(default = "default_actions_negatives")]
    pub actions_negatives: Vec<String>,
    /// Lettre attribuée aux semaines paires ("A" ou "B").
    #[serde(default = "default_semaine_paire")]
    pub semaine_paire: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            actions_positives: default_actions_positives(),
            actions_negatives: default_actions_negatives(),
            semaine_paire: default_semaine_paire(),
        }
    }
}

fn default_actions_positives() -> Vec<String> {
    [
        "Participation",
        "Bonne réponse",
        "Travail soigné",
        "Aide un camarade",
        "Esprit positif",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_actions_negatives() -> Vec<String> {
    [
        "Bavardage",
        "Oubli de matériel",
        "Travail non fait",
        "Attitude inappropriée",
        "Bruit",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_semaine_paire() -> String {
    "A".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub config: Config,
    #[serde(default)]
    pub classes: Vec<Classe>,
    #[serde(default)]
    pub plans: Vec<Plan>,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub edt: Edt,
}

/// Représentation d'un créneau EDT tel que reçu du client (id facultatif).
#[derive(Debug, Clone, Deserialize)]
pub struct CreneauInput {
    #[serde(default)]
    pub id: Option<String>,
    pub jour: u8,
    pub debut: u32,
    #[serde(default)]
    pub duree: u32,
    pub classe_id: String,
    #[serde(default)]
    pub matiere: String,
    #[serde(default)]
    pub salle: String,
}

/// JSON d'import Pronote : { generated, classe, students: [{ prenom, nom, photo }] }.
#[derive(Debug, Clone, Deserialize)]
pub struct PronoteImport {
    #[serde(default)]
    pub classe: String,
    #[serde(default)]
    pub students: Vec<PronoteStudent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PronoteStudent {
    #[serde(default)]
    pub prenom: String,
    #[serde(default)]
    pub nom: String,
    #[serde(default)]
    pub photo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub eleve_id: String,
    #[serde(default)]
    pub texte: String,
    #[serde(default)]
    pub maj: String,
}

#[derive(Debug, Deserialize)]
pub struct EventInput {
    pub eleve_id: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub heure: String,
    pub points: i32,
    #[serde(default)]
    pub libelle: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_par_defaut_has_actions_and_semaine() {
        let cfg = Config::default();
        assert!(!cfg.actions_positives.is_empty());
        assert!(!cfg.actions_negatives.is_empty());
        assert!(cfg.semaine_paire == "A" || cfg.semaine_paire == "B");
    }

    #[test]
    fn data_vide_se_deserialise_de_json_vide() {
        let d: Data = serde_json::from_str("{}").unwrap();
        assert!(d.classes.is_empty() && d.events.is_empty() && d.edt.creneaux.is_empty());
    }

    #[test]
    fn import_pronote_accepte_photo_absente() {
        let p: PronoteImport = serde_json::from_str(r#"{"students":[{"prenom":"A"}]}"#).unwrap();
        assert_eq!(p.classe, "");
        assert!(p.students[0].photo.is_none());
    }

    #[test]
    fn event_sans_crea_reste_deserialisable() {
        let e: Event = serde_json::from_str(
            r#"{"id":"x","classe_id":"c","eleve_id":"e","date":"2025-01-01","points":1,"libelle":"ok"}"#,
        )
        .unwrap();
        assert_eq!(e.crea, 0);
        assert_eq!(e.points, 1);
    }
}
