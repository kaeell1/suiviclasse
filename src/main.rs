mod model;
mod store;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};

use crate::model::{Classe, Config, Edt, Event, EventInput, Note, NoteInput, Plan, PronoteImport, Student, CreneauInput};
use crate::store::Store;

type App = State<Arc<Store>>;

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");

fn erreur(message: impl std::fmt::Display) -> Response {
    (axum::http::StatusCode::BAD_REQUEST, message.to_string()).into_response()
}

fn json_resp<T: serde::Serialize>(valeur: T) -> Response {
    Json(valeur).into_response()
}

fn statique(contenu: &'static str, type_mime: &str) -> Response {
    let type_mime = type_mime.to_string();
    (
        [(header::CONTENT_TYPE, type_mime)],
        contenu,
    )
        .into_response()
}

fn nouvel_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[tokio::main]
async fn main() {
    let chemin = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/suiviclasse.json".to_string());
    let store = Store::shared(chemin.into());
    let hote = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8000);

    let app = construire_routeur(store);

    println!("SuiviClasse démarré : http://{hote}:{port}");

    let listener = tokio::net::TcpListener::bind((hote.as_str(), port))
        .await
        .expect("impossible d'écouter sur le port");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("le serveur s'est arrêté");
}

fn construire_routeur(store: std::sync::Arc<Store>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/classes", get(get_classes).post(post_classe))
        .route("/api/classes/:id", put(put_classe).delete(delete_classe))
        .route("/api/plans/:classe_id", get(get_plan).put(put_plan).delete(delete_plan))
        .route("/api/events/:classe_id", get(get_events).post(post_event))
        .route("/api/events/event/:id", delete(delete_event))
        .route("/api/notes/:classe_id", get(get_notes).put(put_note))
        .route("/api/edt", get(get_edt).put(put_edt))
        .route("/api/edt/import", post(import_edt))
        .route("/api/classes/import-pronote", post(import_pronote))
        .route("/api/export", get(export_all))
        .route("/api/import", post(import_all))
        .with_state(store)
}

async fn index() -> Response {
    statique(INDEX_HTML, "text/html; charset=utf-8")
}

async fn app_js() -> Response {
    statique(APP_JS, "application/javascript; charset=utf-8")
}

async fn style_css() -> Response {
    statique(STYLE_CSS, "text/css; charset=utf-8")
}

async fn get_config(app: App) -> Result<Response, Response> {
    let config = app.read(|d| Ok(d.config.clone())).map_err(erreur)?;
    Ok(json_resp(config))
}

async fn put_config(app: App, Json(config): Json<Config>) -> Result<Response, Response> {
    app.modify(|d| {
        d.config = config;
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn get_classes(app: App) -> Result<Response, Response> {
    let classes = app.read(|d| Ok(d.classes.clone())).map_err(erreur)?;
    Ok(json_resp(classes))
}

async fn post_classe(app: App, Json(mut classe): Json<Classe>) -> Result<Response, Response> {
    if classe.id.trim().is_empty() {
        classe.id = nouvel_id();
    }
    let id = classe.id.clone();
    let recherche = classe.id.clone();
    app.modify(move |d| {
        match d.classes.iter_mut().find(|c| c.id == recherche) {
            Some(c) => *c = classe,
            None => d.classes.push(classe),
        }
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true, "id": id })))
}

async fn put_classe(
    app: App,
    Path(id): Path<String>,
    Json(mut classe): Json<Classe>,
) -> Result<Response, Response> {
    classe.id = id;
    app.modify(|d| {
        match d.classes.iter_mut().find(|c| c.id == classe.id) {
            Some(c) => *c = classe,
            None => d.classes.push(classe),
        }
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn delete_classe(app: App, Path(id): Path<String>) -> Result<Response, Response> {
    app.modify(|d| {
        d.classes.retain(|c| c.id != id);
        d.plans.retain(|p| p.classe_id != id);
        d.events.retain(|e| e.classe_id != id);
        d.notes.retain(|n| n.classe_id != id);
        d.edt.creneaux.retain(|c| c.classe_id != id);
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn import_pronote(
    app: App,
    Json(import): Json<PronoteImport>,
) -> Result<Response, Response> {
    let nom = import.classe.trim().to_string();
    if nom.is_empty() {
        return Err(erreur("nom de classe manquant dans le fichier"));
    }
    let nombre = import.students.len();
    app.modify(|d| {
        let cid = match d.classes.iter().find(|c| c.nom == nom).map(|c| c.id.clone()) {
            Some(id) => id,
            None => {
                let id = nouvel_id();
                d.classes.push(Classe {
                    id: id.clone(),
                    nom: nom.clone(),
                    students: Vec::new(),
                });
                id
            }
        };
        // Mise à jour sans perte : les élèves déjà présents (appariés par
        // prénom + nom) sont conservés avec leur identifiant ; seuls les
        // nouveaux sont ajoutés, et les photos sont rafraîchies.
        for s in import.students {
            let connu = d
                .classes
                .iter()
                .find(|c| c.id == cid)
                .and_then(|c| c.students.iter().find(|e| e.matches_name(&s.prenom, &s.nom)))
                .map(|e| e.id.clone());
            match connu {
                Some(id) => {
                    if s.photo.is_some() {
                        if let Some(c) = d.classes.iter_mut().find(|c| c.id == cid) {
                            if let Some(e) = c.students.iter_mut().find(|e| e.id == id) {
                                e.photo = s.photo;
                            }
                        }
                    }
                }
                None => {
                    if let Some(c) = d.classes.iter_mut().find(|c| c.id == cid) {
                        c.students.push(Student {
                            id: nouvel_id(),
                            prenom: s.prenom,
                            nom: s.nom,
                            photo: s.photo,
                        });
                    }
                }
            }
        }
        Ok(format!("classe « {nom} » : {nombre} élèves importés"))
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true, "eleves": nombre })))
}

async fn get_plan(app: App, Path(classe_id): Path<String>) -> Result<Response, Response> {
    let plan = app
        .read(|d| Ok(d.plans.iter().find(|p| p.classe_id == classe_id).cloned()))
        .map_err(erreur)?;
    Ok(json_resp(plan))
}

async fn put_plan(
    app: App,
    Path(classe_id): Path<String>,
    Json(mut plan): Json<Plan>,
) -> Result<Response, Response> {
    plan.classe_id = classe_id;
    app.modify(|d| {
        match d.plans.iter_mut().find(|p| p.classe_id == plan.classe_id) {
            Some(p) => *p = plan,
            None => d.plans.push(plan),
        }
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn delete_plan(app: App, Path(classe_id): Path<String>) -> Result<Response, Response> {
    app.modify(|d| {
        d.plans.retain(|p| p.classe_id != classe_id);
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn get_events(app: App, Path(classe_id): Path<String>) -> Result<Response, Response> {
    let mut events: Vec<Event> = app
        .read(|d| {
            Ok(d.events
                .iter()
                .filter(|e| e.classe_id == classe_id)
                .cloned()
                .collect())
        })
        .map_err(erreur)?;
    events.sort_by(|a, b| {
        (b.date.as_str(), b.heure.as_str(), b.crea)
            .cmp(&(a.date.as_str(), a.heure.as_str(), a.crea))
    });
    Ok(json_resp(events))
}

async fn post_event(
    app: App,
    Path(classe_id): Path<String>,
    Json(input): Json<EventInput>,
) -> Result<Response, Response> {
    let event = Event {
        id: nouvel_id(),
        classe_id,
        eleve_id: input.eleve_id,
        date: input.date,
        heure: input.heure,
        points: input.points,
        libelle: input.libelle,
        crea: nouveau_crea(),
    };
    let enregistre = event.clone();
    app.modify(|d| {
        d.events.push(event);
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(enregistre))
}

async fn delete_event(app: App, Path(id): Path<String>) -> Result<Response, Response> {
    app.modify(|d| {
        d.events.retain(|e| e.id != id);
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn get_notes(app: App, Path(classe_id): Path<String>) -> Result<Response, Response> {
    let notes: Vec<Note> = app
        .read(|d| {
            Ok(d.notes
                .iter()
                .filter(|n| n.classe_id == classe_id)
                .cloned()
                .collect())
        })
        .map_err(erreur)?;
    Ok(json_resp(notes))
}

async fn put_note(
    app: App,
    Path(classe_id): Path<String>,
    Json(mut input): Json<NoteInput>,
) -> Result<Response, Response> {
    if input.maj.trim().is_empty() {
        input.maj = horodatage();
    }
    input.texte = input.texte.trim().to_string();
    app.modify(|d| {
        let existante = d
            .notes
            .iter_mut()
            .find(|n| n.classe_id == classe_id && n.eleve_id == input.eleve_id);
        match existante {
            Some(n) => {
                n.texte = input.texte.clone();
                n.maj = input.maj.clone();
            }
            None => d.notes.push(Note {
                classe_id,
                eleve_id: input.eleve_id.clone(),
                texte: input.texte,
                maj: input.maj,
            }),
        }
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

async fn get_edt(app: App) -> Result<Response, Response> {
    let edt = app.read(|d| Ok(d.edt.clone())).map_err(erreur)?;
    Ok(json_resp(edt))
}

async fn put_edt(app: App, Json(input): Json<Vec<CreneauInput>>) -> Result<Response, Response> {
    let mut creneaux: Vec<model::Creneau> = input
        .into_iter()
        .map(|c| model::Creneau {
            id: c.id.unwrap_or_else(nouvel_id),
            jour: c.jour.min(6) as u8,
            debut: c.debut,
            duree: if c.duree == 0 { 55 } else { c.duree },
            classe_id: c.classe_id,
            matiere: c.matiere,
            salle: c.salle,
        })
        .collect();
    creneaux.sort_by_key(|c| (c.jour, c.debut));
    app.modify(|d| {
        d.edt = Edt { creneaux };
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(serde_json::json!({ "ok": true })))
}

/// Import d'un fichier EDT (exemple fourni dans le dépôt) : accepte soit un
/// tableau de créneaux, soit un objet { creneaux: [...] }. Les classes d'un
/// créneau sont créées automatiquement si elles n'existent pas encore.
async fn import_edt(app: App, Json(json): Json<serde_json::Value>) -> Result<Response, Response> {
    let creneaux_json = match json {
        serde_json::Value::Array(v) => v,
        serde_json::Value::Object(ref m) => match m.get("creneaux") {
            Some(serde_json::Value::Array(v)) => v.clone(),
            _ => return Err(erreur("format EDT non reconnu : tableau attendu")),
        },
        _ => return Err(erreur("format EDT non reconnu : tableau attendu")),
    };

    let mut a_creer: Vec<model::Creneau> = Vec::new();
    for c in &creneaux_json {
        let obj = match c.as_object() {
            Some(o) => o,
            None => continue,
        };
        let champ = |noms: &[&str]| -> Option<String> {
            noms.iter().find_map(|n| obj.get(*n)).and_then(|v| match v {
                serde_json::Value::Null => None,
                autre => Some(autre.to_string().trim_matches('"').to_string()),
            })
        };
        let jour_texte = champ(&["jour", "jourSemaine", "day"]).unwrap_or_default();
        let jour = match jour_texte.to_lowercase().as_str() {
            "lundi" | "1" => 0,
            "mardi" | "2" => 1,
            "mercredi" | "3" => 2,
            "jeudi" | "4" => 3,
            "vendredi" | "5" => 4,
            "samedi" | "6" => 5,
            "dimanche" | "0" | "7" => 6,
            _ => 0,
        };
        let debut = champ(&["debut", "heure", "horaire", "start"]).unwrap_or_default();
        let debut_min = match parse_heure(&debut) {
            Some(m) => m,
            None => continue,
        };
        let duree = champ(&["duree", "durée", "duration"])
            .and_then(|t| t.trim().parse::<u32>().ok())
            .or_else(|| champ(&["dureeFin", "fin"]).and_then(|t| parse_heure(&t)))
            .unwrap_or(55)
            .max(1);
        let classe_nom = champ(&["classe", "classeNom", "groupe"]).unwrap_or_default();
        if classe_nom.is_empty() {
            continue;
        }
        a_creer.push(model::Creneau {
            id: nouvel_id(),
            jour,
            debut: debut_min,
            duree,
            classe_id: classe_nom,
            matiere: champ(&["matiere", "matière", "discipline", "subject"]).unwrap_or_default(),
            salle: champ(&["salle", "room"]).unwrap_or_default(),
        });
    }

    let nombre = a_creer.len();
    app.modify(move |d| {
        for mut c in a_creer {
            let existant = d.classes.iter().find(|k| k.nom == c.classe_id).map(|k| k.id.clone());
            c.classe_id = match existant {
                Some(id) => id,
                None => {
                    let id = nouvel_id();
                    d.classes.push(Classe {
                        id: id.clone(),
                        nom: c.classe_id.clone(),
                        students: Vec::new(),
                    });
                    id
                }
            };
            d.edt.creneaux.push(c);
        }
        d.edt.creneaux.sort_by_key(|c| (c.jour, c.debut));
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(
        serde_json::json!({ "ok": true, "ajoutes": nombre }),
    ))
}

fn parse_heure(texte: &str) -> Option<u32> {
    let t = texte.trim().replacen('h', ":", 1);
    if let Some((h, m)) = t.split_once(':') {
        Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?)
    } else {
        Some(t.trim().parse::<u32>().ok()? * 60)
    }
}

fn nouveau_crea() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn horodatage() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (jours, reste) = (now / 86400, now % 86400);
    format!("{jours}j {h:02}:{m:02}", h = reste / 3600, m = (reste % 3600) / 60)
}

async fn export_all(app: App) -> Result<Response, Response> {
    let data = app.read(|d| Ok(d.clone())).map_err(erreur)?;
    let corps = serde_json::to_string(&serde_json::json!({
        "app": "suiviclasse-rs",
        "version": 1,
        "config": data.config,
        "classes": data.classes,
        "plans": data.plans,
        "events": data.events,
        "notes": data.notes,
        "edt": data.edt,
    }))
    .map_err(erreur)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"suiviclasse-export.json\"".to_string(),
            ),
        ],
        corps,
    )
        .into_response())
}

async fn import_all(app: App, Json(nouvelles): Json<model::Data>) -> Result<Response, Response> {
    let (n_classes, n_events) = (nouvelles.classes.len(), nouvelles.events.len());
    app.modify(|d| {
        for classe in nouvelles.classes {
            match d.classes.iter_mut().find(|c| c.id == classe.id) {
                Some(c) => *c = classe,
                None => d.classes.push(classe),
            }
        }
        for plan in nouvelles.plans {
            match d.plans.iter_mut().find(|p| p.classe_id == plan.classe_id) {
                Some(p) => *p = plan,
                None => d.plans.push(plan),
            }
        }
        d.events.extend(nouvelles.events);
        for note in nouvelles.notes {
            match d
                .notes
                .iter_mut()
                .find(|n| n.classe_id == note.classe_id && n.eleve_id == note.eleve_id)
            {
                Some(n) => *n = note,
                None => d.notes.push(note),
            }
        }
        if !nouvelles.edt.creneaux.is_empty() {
            d.edt = nouvelles.edt;
        }
        Ok(())
    })
    .map_err(erreur)?;
    Ok(json_resp(
        serde_json::json!({ "ok": true, "classes": n_classes, "events": n_events }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn app_test() -> Router {
        let suffixe = uuid::Uuid::new_v4().simple().to_string();
        let chemin = std::env::temp_dir().join(format!("suiviclasse-test-{suffixe}.json"));
        construire_routeur(Store::shared(chemin))
    }

    async fn requete(app: Router, methode: &str, chemin: &str, corps: Option<&str>) -> (StatusCode, serde_json::Value) {
        let builder = Request::builder().method(methode).uri(chemin);
        let demande = match corps {
            Some(c) => builder.header("content-type", "application/json").body(Body::from(c.to_string())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        let reponse = app.oneshot(demande).await.unwrap();
        let statut = reponse.status();
        let json_attendu = reponse
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.starts_with("application/json"))
            .unwrap_or(false);
        let octets = axum::body::to_bytes(reponse.into_body(), usize::MAX).await.unwrap();
        let valeur = if json_attendu && !octets.is_empty() {
            serde_json::from_slice(&octets).expect("réponse JSON invalide")
        } else {
            serde_json::Value::Null
        };
        (statut, valeur)
    }

    #[test]
    fn parse_heure_reconnait_les_formats() {
        assert_eq!(parse_heure("08:00"), Some(480));
        assert_eq!(parse_heure("8h30"), Some(510));
        assert_eq!(parse_heure("14"), Some(840));
        assert_eq!(parse_heure(" 9:05 "), Some(545));
        assert_eq!(parse_heure("abc"), None);
    }

    #[tokio::test]
    async fn accueil_et_fichiers_statiques_sont_servis() {
        let app = app_test();
        for chemin in ["/", "/app.js", "/style.css"] {
            let (statut, _) = requete(app.clone(), "GET", chemin, None).await;
            assert_eq!(statut, StatusCode::OK, "échec sur {chemin}");
        }
    }

    #[tokio::test]
    async fn configuration_par_defaut_lisible_et_modifiable() {
        let app = app_test();
        let (statut, config) = requete(app.clone(), "GET", "/api/config", None).await;
        assert_eq!(statut, StatusCode::OK);
        assert!(!config["actions_positives"].as_array().unwrap().is_empty());

        let (statut, _) = requete(app.clone(), "PUT", "/api/config", Some(r#"{"actions_positives":["Participation"],"actions_negatives":["Bruit"],"semaine_paire":"B"}"#)).await;
        assert_eq!(statut, StatusCode::OK);
        let (_, config) = requete(app, "GET", "/api/config", None).await;
        assert_eq!(config["semaine_paire"], "B");
        assert_eq!(config["actions_negatives"][0], "Bruit");
    }

    #[tokio::test]
    async fn import_pronote_puis_suivi_complet() {
        let app = app_test();
        let (statut, _) = requete(app.clone(), "POST", "/api/classes/import-pronote", Some(
            r#"{"classe":"6e A","students":[{"prenom":"Jean","nom":"Dupont"},{"prenom":"Marie","nom":"Martin","photo":"data:img;base64,AAA"}]}"#,
        )).await;
        assert_eq!(statut, StatusCode::OK);

        let (_, classes) = requete(app.clone(), "GET", "/api/classes", None).await;
        assert_eq!(classes.as_array().unwrap().len(), 1);
        let classe = &classes[0];
        let cid = classe["id"].as_str().unwrap();
        assert_eq!(classe["students"].as_array().unwrap().len(), 2);

        // ré-import : pas de doublon, photo rafraîchie
        requete(app.clone(), "POST", "/api/classes/import-pronote", Some(
            r#"{"classe":"6e A","students":[{"prenom":"Jean","nom":"Dupont","photo":"data:img;base64,BBB"},{"prenom":"Zoé","nom":"Bernard"}]}"#,
        )).await;
        let (_, classes) = requete(app.clone(), "GET", "/api/classes", None).await;
        let eleves = classes[0]["students"].as_array().unwrap();
        assert_eq!(eleves.len(), 3, "ré-import doit ajouter Zoé sans dupliquer Jean");
        let jean = eleves.iter().find(|e| e["prenom"] == "Jean").unwrap();
        assert_eq!(jean["photo"], "data:img;base64,BBB");

        // plan
        let plan = format!(
            r#"{{"classe_id":"{cid}","nom":"standard","tables":[{{"col":1,"row":1,"seats":["{}", null]}}]}}"#,
            jean["id"].as_str().unwrap()
        );
        let (statut, _) = requete(app.clone(), "PUT", &format!("/api/plans/{cid}"), Some(&plan)).await;
        assert_eq!(statut, StatusCode::OK);
        let (_, plan) = requete(app.clone(), "GET", &format!("/api/plans/{cid}"), None).await;
        assert_eq!(plan["tables"][0]["seats"][0], jean["id"]);

        // événements : ordre des plus récents d'abord
        for (points, libelle) in [(1, "Participation"), (-1, "Bavardage"), (2, "Custom")] {
            let corps = format!(r#"{{"eleve_id":"{}","date":"2025-11-21","heure":"08:15","points":{points},"libelle":"{libelle}"}}"#, jean["id"].as_str().unwrap());
            let (statut, _) = requete(app.clone(), "POST", &format!("/api/events/{cid}"), Some(&corps)).await;
            assert_eq!(statut, StatusCode::OK);
        }
        let (_, events) = requete(app.clone(), "GET", &format!("/api/events/{cid}"), None).await;
        assert_eq!(events[0]["libelle"], "Custom");

        // suppression d'une action
        let id = events[1]["id"].as_str().unwrap();
        let (statut, _) = requete(app.clone(), "DELETE", &format!("/api/events/event/{id}"), None).await;
        assert_eq!(statut, StatusCode::OK);
        let (statut, _) = requete(app.clone(), "DELETE", &format!("/api/events/event/{id}"), None).await;
        assert_eq!(statut, StatusCode::OK, "suppression idempotente");

        // note pense-bête
        let marie = classes[0]["students"][1]["id"].as_str().unwrap();
        let corps_note = format!(r#"{{"eleve_id":"{marie}","texte":"x"}}"#);
        let (statut, _) = requete(app.clone(), "PUT", &format!("/api/notes/{cid}"), Some(&corps_note)).await;
        assert_eq!(statut, StatusCode::OK);
        let (_, notes) = requete(app.clone(), "GET", &format!("/api/notes/{cid}"), None).await;
        assert_eq!(notes[0]["texte"], "x");
        assert!(!notes[0]["maj"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn import_edt_cree_les_classes_et_trie() {
        let app = app_test();
        let (statut, rep) = requete(app.clone(), "POST", "/api/edt/import", Some(
            r#"[{"jour":"Vendredi","heure":"14:00","duree":55,"classe":"4e C","matiere":"Techno","salle":"Info 1"},
                {"jour":"Lundi","heure":"08:00","duree":55,"classe":"6e A","matiere":"Maths","salle":"101"}]"#,
        )).await;
        assert_eq!(statut, StatusCode::OK);
        assert_eq!(rep["ajoutes"], 2);

        let (_, classes) = requete(app.clone(), "GET", "/api/classes", None).await;
        let noms: Vec<&str> = classes.as_array().unwrap().iter().map(|c| c["nom"].as_str().unwrap()).collect();
        assert!(noms.contains(&"4e C") && noms.contains(&"6e A"));

        let (_, edt) = requete(app, "GET", "/api/edt", None).await;
        let creneaux = edt["creneaux"].as_array().unwrap();
        assert_eq!(creneaux.len(), 2);
        assert!(creneaux[0]["debut"].as_u64().unwrap() < creneaux[1]["debut"].as_u64().unwrap(), "créneaux triés par jour puis heure");
    }

    #[tokio::test]
    async fn export_contient_tout_et_import_le_restitute() {
        let app = app_test();
        requete(app.clone(), "POST", "/api/classes", Some(r#"{"id":"","nom":"Test","students":[]}"#)).await;
        let (statut, export) = requete(app.clone(), "GET", "/api/export", None).await;
        assert_eq!(statut, StatusCode::OK);
        assert_eq!(export["app"], "suiviclasse-rs");
        assert_eq!(export["classes"].as_array().unwrap().len(), 1);

        let (statut, _) = requete(app, "POST", "/api/import", Some(&export.to_string())).await;
        assert_eq!(statut, StatusCode::OK);
    }
}
