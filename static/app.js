"use strict";

/* ================= utilitaires ================= */

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => [...document.querySelectorAll(sel)];

function esc(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

function nouvelId() {
  return (crypto.randomUUID ? crypto.randomUUID().replaceAll("-", "") : Date.now() + "-" + Math.random().toString(36).slice(2));
}

async function api(chemin, options = {}) {
  const { corps, methode = corps ? "POST" : "GET", ...reste } = options;
  const init = { method: methode, ...reste };
  if (corps !== undefined) {
    init.headers = { "Content-Type": "application/json", ...(init.headers || {}) };
    init.body = JSON.stringify(corps);
  }
  const reponse = await fetch(chemin, init);
  const texte = await reponse.text();
  const donnees = texte ? JSON.parse(texte) : null;
  if (!reponse.ok) throw new Error((donnees && donnees.message) || reponse.statusText);
  return donnees;
}

function aujourdhui() {
  return new Date().toISOString().slice(0, 10);
}

function heureActuelle() {
  const d = new Date();
  return String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
}

function numeroSemaine(dateTexte) {
  const t = new Date(dateTexte + "T00:00:00Z");
  const jour = t.getUTCDay() || 7;
  t.setUTCDate(t.getUTCDate() + 4 - jour);
  const debut = new Date(Date.UTC(t.getUTCFullYear(), 0, 1));
  return Math.ceil(((t - debut) / 86400000 + 1) / 7);
}

const JOURS = ["Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi"];

/* ================= état ================= */

const etat = {
  config: { actions_positives: [], actions_negatives: [], semaine_paire: "A" },
  classes: [],
  events: [],
  notes: [],
  plan: null,
  classeSuivantPlan: null,
  selectionEleve: null,
  cible: null,
  dateSuivi: aujourdhui(),
  onglet: "seance",
};

let creneaux = [];

/* ================= dispositions prédéfinies ================= */

function grille(nTables, nbSieges, colonnes) {
  const t = [];
  for (let i = 0; i < nTables; i++) {
    t.push({ col: (i % colonnes) + 1, row: Math.floor(i / colonnes) + 1, seats: Array(nbSieges).fill(null) });
  }
  return t;
}

const PRESETS = {
  standard: () => grille(12, 2, 3),
  complete: () => grille(18, 2, 3),
  u: () => {
    const t = [];
    for (let r = 1; r <= 4; r++) { t.push({ col: 1, row: r, seats: [null, null] }); t.push({ col: 6, row: r, seats: [null, null] }); }
    for (let c = 2; c <= 5; c++) t.push({ col: c, row: 5, seats: [null, null] });
    return t;
  },
  ilots: () => {
    const t = [];
    for (let g = 0; g < 3; g++)
      for (const cc of [0, 1])
        for (const r of [0, 1])
          t.push({ col: g * 3 + cc + 1, row: r + 1, seats: [null, null] });
    return t;
  },
  sciences: () => grille(6, 3, 2),
  ds: () => grille(18, 1, 3),
};

/* ================= onglets ================= */

function montrerOnglet(nom) {
  etat.onglet = nom;
  for (const s of $$(".onglet")) s.hidden = s.id !== "onglet-" + nom;
  for (const b of $$("#onglets button")) b.classList.toggle("actif", b.dataset.onglet === nom);
  if (nom === "plan") chargerVuePlan();
  if (nom === "suivi") chargerVueSuivi();
  if (nom === "seance") chargerVueSeance();
  if (nom === "edt") rendreEdt();
}

/* ================= classes ================= */

function classeParId(id) {
  return etat.classes.find((c) => c.id === id);
}

function eleveParId(id) {
  for (const c of etat.classes)
    for (const e of c.students)
      if (e.id === id) return e;
  return null;
}

async function chargerClasses() {
  etat.classes = await api("/api/classes");
  for (const sel of ["#seance-classe", "#plan-classe", "#suivi-classe", "#suppr-classe"]) {
    const el = $(sel);
    const conservé = el.value;
    el.innerHTML = '<option value="">— choisir —</option>' +
      etat.classes.map((c) => `<option value="${esc(c.id)}">${esc(c.nom)}</option>`).join("");
    if (etat.classes.some((c) => c.id === conservé)) el.value = conservé;
  }
}

/* ================= points ================= */

function pointsDe(eleveId) {
  return etat.events.filter((e) => e.eleve_id === eleveId).reduce((total, e) => total + e.points, 0);
}

function badgePoints(total) {
  const t = Number(total);
  if (t > 0) return `<span class="points haut">+${t}</span>`;
  if (t < 0) return `<span class="points bas">${t}</span>`;
  return `<span class="points neutre">0</span>`;
}

/* ================= plan de classe ================= */

function tablePlanVide(preset) {
  return { classe_id: "", nom: "standard", tables: PRESETS[preset || "standard"]() };
}

function siegeRenferme(plan, eleveId) {
  for (const t of plan.tables)
    for (let i = 0; i < t.seats.length; i++)
      if (t.seats[i] === eleveId) return true;
  return false;
}

function nonAssignes(classeId) {
  const classe = classeParId(classeId);
  if (!classe || !etat.plan) return [];
  return classe.students.filter((e) => !siegeRenferme(etat.plan, e.id));
}

function carteEleve(e, points) {
  const image = e.photo
    ? `<img src="${esc(e.photo)}" alt="" draggable="${points === null ? "false" : "true"}">`
    : `<span class="initiales">${esc(initiales(e))}</span>`;
  const pt = points === null ? "" : badgePoints(points);
  return `${image}<span class="nom">${esc(e.prenom)}<small>${esc(e.nom)}</small></span>${pt}`;
}

function initiales(e) {
  return ((e.prenom || " ")[0] + (e.nom || " ")[0] || "?").toUpperCase();
}

function linote(classeId, eleveId) {
  const n = etat.notes.find((n) => n.classe_id === classeId && n.eleve_id === eleveId);
  return n && n.texte ? n : null;
}

function rendrePlan(conteneur, mode) {
  conteneur.innerHTML = "";
  if (!etat.plan) {
    conteneur.innerHTML = `<p>Aucun plan de classe pour cette classe. Créez-le dans l'onglet « Plan de classe ».</p>`;
    return;
  }
  const liste = mode === "edition" ? $("#plan-non-assignes") : null;
  if (liste) {
    liste.innerHTML = "";
    const eleves = nonAssignes(etat.plan.classe_id);
    const titre = document.createElement("div");
    titre.innerHTML = `<small>${eleves.length === 0 ? "Tous les élèves sont placés." : "Cliquez sur un élève puis sur un siège, ou glissez-déposez."}</small>`;
    liste.appendChild(titre);
    for (const e of eleves) {
      const c = document.createElement("div");
      c.className = "siege plein";
      c.draggable = true;
      c.dataset.eleveId = e.id;
      c.innerHTML = carteEleve(e, null);
      if (etat.selectionEleve === e.id) c.classList.add("sel");
      c.addEventListener("click", () => {
        etat.selectionEleve = etat.selectionEleve === e.id ? null : e.id;
        const g = $("#plan-grille");
        if (etat.selectionEleve && g) g.addEventListener("pointerdown", () => {}, { once: true });
        rendrePlan(conteneur, mode);
      });
      c.addEventListener("dragstart", (ev) => ev.dataTransfer.setData("text/eleve", e.id));
      liste.appendChild(c);
    }
  }

  const grille = mode === "edition" ? $("#plan-grille") : document.createElement("div");
  if (mode !== "edition") {
    grille.className = "grille-tables";
  } else {
    grille.innerHTML = "";
  }
  const nbColonnes = Math.max(0, ...etat.plan.tables.map((t) => Number(t.col) || 1));
  const nbRangees = Math.max(0, ...etat.plan.tables.map((t) => Number(t.row) || 1));
  grille.style.gridTemplateColumns = `repeat(${nbColonnes * 2}, minmax(0, 1fr))`;
  grille.style.gridAutoRows = "min-content";

  etat.plan.tables.forEach((t, index) => {
    const div = document.createElement("div");
    div.className = "table";
    div.style.setProperty("--col", (t.col - 1) * 2 + 1);
    div.style.setProperty("--row", t.row);
    div.style.gridColumn = `${(t.col - 1) * 2 + 1} / span 2`;
    div.style.gridRow = t.row;
    const nbParLigne = t.seats.length >= 3 ? 2 : t.seats.length;
    div.style.gridTemplateColumns = `repeat(${nbParLigne}, 1fr)`;
    const numero = document.createElement("div");
    numero.className = "table-num";
    numero.textContent = "Table " + (index + 1);
    div.appendChild(numero);
    t.seats.forEach((eleveId, sIndex) => {
      const siege = document.createElement("div");
      siege.className = "siege";
      siege.dataset.table = index;
      siege.dataset.siege = sIndex;
      const e = eleveId ? eleveParId(eleveId) : null;
      if (e) {
        siege.classList.add("plein");
        siege.draggable = mode === "edition";
        siege.innerHTML = carteEleve(e, mode === "edition" ? null : pointsDe(e.id));
        const note = linote(etat.plan.classe_id, e.id);
        if (note && mode !== "edition") {
          siege.classList.add("pointe");
          siege.title = "Note : " + note.texte;
          const p = note.texte.length > 1 ? " ✎" : "";
          siege.insertAdjacentHTML("beforeend", `<span class="snote${p === "" ? "" : p.trim() ? " y" : ""}">${p}</span>`);
        }
      } else {
        siege.innerHTML = `<span class="nu">+ siège libre</span>`;
      }
      siege.addEventListener("click", () => clicSiege(index, sIndex, mode));
      siege.addEventListener("dblclick", () => dblClicSiege(index, sIndex, mode));
      siege.addEventListener("dragover", (ev) => ev.preventDefault());
      siege.addEventListener("drop", (ev) => {
        ev.preventDefault();
        const id = ev.dataTransfer.getData("text/eleve");
        if (id) placerEleve(id, index, sIndex);
      });
      siege.addEventListener("dragstart", (ev) => {
        const id = etat.plan.tables[index].seats[sIndex];
        if (id) ev.dataTransfer.setData("text/eleve", id);
      });
      div.appendChild(siege);
    });
    grille.appendChild(div);
  });
  if (mode !== "edition") conteneur.appendChild(grille);
}

function clicSiege(index, sIndex, mode) {
  const t = etat.plan.tables[index];
  const idActuel = t.seats[sIndex];
  if (mode === "edition") {
    if (etat.selectionEleve) {
      placerEleve(etat.selectionEleve, index, sIndex);
      etat.selectionEleve = null;
    }
    rendrePlan(document.querySelector(".zone-plan"), "edition");
    return;
  }
  if (idActuel) ouvrirModalActions(idActuel);
}

function dblClicSiege(index, sIndex, mode) {
  const t = etat.plan.tables[index];
  const idActuel = t.seats[sIndex];
  if (mode === "edition") {
    if (idActuel) {
      t.seats[sIndex] = null;
      enregistrerPlan();
      rendrePlan(document.querySelector(".zone-plan"), "edition");
    }
    return;
  }
  if (idActuel) ouvrirModalActions(idActuel, true);
}

async function placerEleve(id, index, sIndex) {
  const t = etat.plan.tables[index];
  for (const tt of etat.plan.tables)
    tt.seats = tt.seats.map((s) => (s === id ? null : s));
  t.seats[sIndex] = id;
  await enregistrerPlan();
  etat.selectionEleve = null;
  rendrePlan(document.querySelector(".zone-plan"), "edition");
}

async function enregistrerPlan() {
  try {
    await api("/api/plans/" + etat.plan.classe_id, { methode: "PUT", corps: etat.plan });
    afficherMessagePlan("Plan enregistré ✔");
  } catch (err) {
    afficherMessagePlan("Erreur d'enregistrement : " + err.message);
  }
}

function afficherMessagePlan(texte) {
  const el = $("#plan-message");
  if (el) el.textContent = texte;
}

/* ---------- vues ---------- */

async function chargerVuePlan() {
  const classeId = $("#plan-classe").value;
  if (!classeId) {
    etat.plan = null;
    $("#plan-non-assignes").innerHTML = "";
    $("#plan-grille").innerHTML = "";
    return;
  }
  const plan = await api("/api/plans/" + classeId);
  if (!plan) {
    etat.plan = { classe_id: classeId, nom: "standard", tables: PRESETS.standard() };
    afficherMessagePlan("Disposition standard proposée : modifiez puis enregistrez.");
  } else {
    etat.plan = plan;
    afficherMessagePlan("");
  }
  rendrePlan($("#plan-zone"), "edition");
}

async function chargerVueSuivi() {
  await chargerEvenements($("#suivi-classe").value);
  const zone = $("#suivi-zone");
  zone.innerHTML = "";
  const contenu = document.createElement("div");
  zone.appendChild(contenu);
  etat.plan = null;
  const classeId = $("#suivi-classe").value;
  if (classeId) {
    etat.plan = await api("/api/plans/" + classeId);
  }
  rendrePlan(contenu, "suivi");
  rendreSynthese();
  rendreHistSuivi();
}

function rendreSynthese() {
  const corps = $("#suivi-synthese tbody");
  const classeId = $("#suivi-classe").value;
  const classe = classeParId(classeId);
  corps.innerHTML = "";
  if (!classe) return;
  for (const e of classe.students) {
    const ev = etat.events.filter((x) => x.eleve_id === e.id);
    const total = ev.reduce((t, x) => t + x.points, 0);
    const note = linote(classeId, e.id);
    corps.insertAdjacentHTML("beforeend", `<tr><td>${esc(e.prenom)} ${esc(e.nom)}</td>
      <td>${total}</td>
      <td>${ev.filter((x) => x.points > 0).length}</td>
      <td>${ev.filter((x) => x.points < 0).length}</td>
      <td>${note ? esc(note.texte) : ""}</td></tr>`);
  }
}

function rendreHistSuivi() {
  const ul = $("#suivi-historique");
  ul.innerHTML = "";
  for (const ev of etat.events.slice(0, 40)) {
    const e = eleveParId(ev.eleve_id);
    ul.insertAdjacentHTML("beforeend", `<li>
      <span class="${ev.points >= 0 ? "plus" : "moins"}">${ev.points > 0 ? "+" : ""}${ev.points}</span>
      <strong>${esc(e ? e.prenom + " " + e.nom : "élève supprimé")}</strong> — ${esc(ev.libelle)}
      <span class="date">${esc(ev.date)} ${esc(ev.heure || "")}</span>
      <button data-suppr="${esc(ev.id)}" title="Supprimer">🗑</button>
    </li>`);
  }
  for (const b of ul.querySelectorAll("[data-suppr]"))
    b.addEventListener("click", async () => {
      await api("/api/events/event/" + b.dataset.suppr, { methode: "DELETE" });
      await chargerVueSuivi();
    });
}

async function chargerVueSeance() {
  const classeId = $("#seance-classe").value;
  $("#seance-semaine").textContent = "";
  const contenu = $("#seance-zone");
  contenu.innerHTML = "";
  if (!classeId) {
    contenu.innerHTML = "<p>pas de classe chargée</p>";
    return;
  }
  const date = $("#seance-date").value || aujourdhui();
  const n = numeroSemaine(date);
  const lettre = (n % 2 === 0) === (etat.config.semaine_paire === "A") ? "A" : "B";
  $("#seance-semaine").textContent = `Semaine ${n} ${lettre}`;
  etat.plan = await api("/api/plans/" + classeId);
  rendrePlan(contenu, "suivi");
}

/* ---------- événements de séance / suivi ---------- */

async function chargerEvenements(classeId) {
  etat.events = classeId ? await api("/api/events/" + classeId) : [];
  etat.notes = classeId ? await api("/api/notes/" + classeId) : [];
}

/* ================= modale actions / note ================= */

function voile(montrer) {
  $("#voile").hidden = !montrer;
}

function fermerModales() {
  voile(false);
  $("#modal-actions").hidden = true;
  etat.cible = null;
}

function ouvrirModalActions(eleveId, focusNote = false) {
  const e = eleveParId(eleveId);
  const classeId = etat.plan ? etat.plan.classe_id : "";
  if (!e) return;
  etat.cible = eleveId;
  $("#modal-actions").hidden = false;
  voile(true);
  $("#modal-actions-titre").textContent = "Gestion des points pour " + e.prenom + " " + e.nom;
  const total = pointsDe(eleveId);
  const nb = etat.events.filter((x) => x.eleve_id === eleveId).length;
  $("#modal-actions-points").textContent = `${total} point${Math.abs(total) === 1 ? "" : "s"} au total — ${nb} action${nb === 1 ? "" : "s"}`;
  const remplir = (conteneur, liste, delta) => {
    conteneur.innerHTML = "";
    for (const libelle of liste) {
      const b = document.createElement("button");
      b.className = delta > 0 ? "pos" : "neg";
      b.textContent = libelle;
      b.addEventListener("click", () => ajouterAction(delta, libelle));
      conteneur.appendChild(b);
    }
  };
  remplir($("#modal-positives"), etat.config.actions_positives, 1);
  remplir($("#modal-negatives"), etat.config.actions_negatives, -1);
  const note = linote(classeId, eleveId);
  $("#modal-note").value = note ? note.texte : "";
  rendreHistModal();
  if (focusNote) $("#modal-note").focus();
}

function rendreHistModal() {
  const ul = $("#modal-historique");
  ul.innerHTML = "";
  const evs = etat.events.filter((e) => e.eleve_id === etat.cible).slice(0, 20);
  if (evs.length === 0) ul.innerHTML = "<li>Aucune action enregistrée.</li>";
  for (const ev of evs) {
    ul.insertAdjacentHTML("beforeend", `<li>
      <span class="${ev.points >= 0 ? "plus" : "moins"}">${ev.points > 0 ? "+" : ""}${ev.points}</span>
      ${esc(ev.libelle)}
      <span class="date">${esc(ev.date)} ${esc(ev.heure || "")}</span>
      <button data-suppr="${esc(ev.id)}">🗑</button>
    </li>`);
  }
  for (const b of ul.querySelectorAll("[data-suppr]"))
    b.addEventListener("click", async () => {
      await api("/api/events/event/" + b.dataset.suppr, { methode: "DELETE" });
      await rafraichirApresAction();
    });
}

async function ajouterAction(points, libelle) {
  if (!etat.cible || !libelle) return;
  let date = etat.dateSuivi;
  if (etat.onglet === "seance") date = $("#seance-date").value || aujourdhui();
  if (etat.onglet === "plan" || !date) date = aujourdhui();
  await api("/api/events/" + (etat.plan ? etat.plan.classe_id : ""), {
    methode: "POST",
    corps: { eleve_id: etat.cible, date, heure: heureActuelle(), points, libelle },
  });
  await rafraichirApresAction();
}

async function rafraichirApresAction() {
  const classeId = etat.plan ? etat.plan.classe_id : "";
  await chargerEvenements(classeId);
  if (etat.cible) {
    const e = eleveParId(etat.cible);
    const total = pointsDe(etat.cible);
    const nb = etat.events.filter((x) => x.eleve_id === etat.cible).length;
    $("#modal-actions-points").textContent = `${total} point${Math.abs(total) === 1 ? "" : "s"} au total — ${nb} action${nb === 1 ? "" : "s"}`;
    rendreHistModal();
  }
  const zone = etat.onglet === "plan" ? null : etat.onglet === "seance" ? $("#seance-zone") : $("#suivi-zone");
  if (zone) rendrePlan(zone, etat.onglet === "plan" ? "edition" : "suivi");
  if (etat.onglet === "suivi") { rendreSynthese(); rendreHistSuivi(); }
}

async function enregistrerNote() {
  if (!etat.cible) return;
  const classeId = etat.plan ? etat.plan.classe_id : "";
  await api("/api/notes/" + classeId, {
    methode: "PUT",
    corps: { eleve_id: etat.cible, texte: $("#modal-note").value, maj: new Date().toISOString().slice(0, 10) },
  });
  await rafraichirApresAction();
}

/* ================= EDT ================= */

function rendreEdt() {
  const corps = $("#edt-corps");
  corps.innerHTML = "";
  creneaux.forEach((c, i) => {
    const classe = classeParId(c.classe_id);
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td><select data-i="${i}" data-champ="jour">
        ${JOURS.map((j, k) => `<option value="${k}" ${k === c.jour ? "selected" : ""}>${j}</option>`).join("")}
      </select></td>
      <td><input type="time" data-i="${i}" data-champ="debut" value="${minutesVersHeure(c.debut)}"></td>
      <td><input type="number" data-i="${i}" data-champ="duree" min="5" step="5" value="${c.duree}"></td>
      <td><select data-i="${i}" data-champ="classe_id">
        ${etat.classes.map((k) => `<option value="${esc(k.id)}" ${k.id === c.classe_id ? "selected" : ""}>${esc(k.nom)}</option>`).join("")}
      </select></td>
      <td><input type="text" data-i="${i}" data-champ="matiere" value="${esc(c.matiere)}"></td>
      <td><input type="text" data-i="${i}" data-champ="salle" value="${esc(c.salle)}"></td>
      <td><button data-suppr-ligne="${i}" class="danger">🗑</button></td>`;
    const nomMatiere = c.classe_id && !classe ? esc(c.classe_id) + " (classe supprimée)" : "";
    if (nomMatiere) tr.cells[3].appendChild(Object.assign(document.createElement("em"), { textContent: " " + nomMatiere }));
    corps.appendChild(tr);
  });
  for (const el of corps.querySelectorAll("[data-champ]"))
    el.addEventListener("change", majCreneauLigne);
  for (const b of corps.querySelectorAll("[data-suppr-ligne]"))
    b.addEventListener("click", () => { creneaux.splice(Number(b.dataset.supprLigne), 1); rendreEdt(); });
}

function majCreneauLigne(ev) {
  const i = Number(ev.target.dataset.i);
  const champ = ev.target.dataset.champ;
  let valeur = ev.target.value;
  if (champ === "debut") valeur = heureVersMinutes(valeur);
  if (champ === "duree" || champ === "jour") valeur = Number(valeur) || 0;
  creneaux[i][champ] = valeur;
}

function minutesVersHeure(m) {
  return String(Math.floor(m / 60)).padStart(2, "0") + ":" + String(m % 60).padStart(2, "0");
}

function heureVersMinutes(h) {
  if (!h) return 480;
  const [a, b] = h.split(":");
  return Number(a) * 60 + Number(b || 0);
}

async function enregistrerEdt() {
  await api("/api/edt", { methode: "PUT", corps: creneaux });
  await remplirChoixSeance();
  alert("EDT enregistré.");
}

async function remplirChoixSeance() {
  const edt = await api("/api/edt");
  creneaux = edt.creneaux || [];
  const selJour = $("#seance-jour");
  const selHeure = $("#seance-heure");
  selJour.innerHTML = JOURS.slice(0, 6).map((j, k) => `<option value="${k}">${j}</option>`).join("");
  const her = [...new Set(creneaux.map((c) => c.debut))].sort((a, b) => a - b);
  selHeure.innerHTML = her.map((m) => `<option value="${m}">${minutesVersHeure(m)}</option>`).join("");
  if (!her.length) selHeure.innerHTML = '<option value="">—</option>';
  const cible = her.length ? her[0] : null;
  const correspondant = creneaux.find((c) => c.debut === cible);
  if (correspondant) {
    selJour.value = correspondant.jour;
    selHeure.value = correspondant.debut;
    $("#seance-classe").value = correspondant.classe_id;
  }
  selectionJourHeure();
}

function selectionJourHeure() {
  const selJour = $("#seance-jour");
  const selHeure = $("#seance-heure");
  const c = creneaux.find((k) => k.jour === Number(selJour.value) && k.debut === Number(selHeure.value));
  if (c) {
    if (etat.classes.some((k) => k.id === c.classe_id)) $("#seance-classe").value = c.classe_id;
  }
}

/* ================= configuration ================= */

function remplirConfiguration() {
  $("#cfg-positives").value = etat.config.actions_positives.join("\n");
  $("#cfg-negatives").value = etat.config.actions_negatives.join("\n");
  $("#cfg-semaine").value = etat.config.semaine_paire;
}

async function enregistrerConfiguration() {
  etat.config = {
    actions_positives: $("#cfg-positives").value.split("\n").map((s) => s.trim()).filter(Boolean),
    actions_negatives: $("#cfg-negatives").value.split("\n").map((s) => s.trim()).filter(Boolean),
    semaine_paire: $("#cfg-semaine").value,
  };
  await api("/api/config", { methode: "PUT", corps: etat.config });
  alert("Configuration enregistrée.");
}

/* ================= import / fichiers ================= */

function lireFichier(fichier, traiter) {
  const r = new FileReader();
  r.onload = () => {
    try { traiter(JSON.parse(r.result)); }
    catch (err) { alert("Fichier JSON invalide : " + err.message); }
  };
  r.readAsText(fichier);
}

function telecharger(nomFichier, donnees) {
  const blob = new Blob([JSON.stringify(donnees, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = nomFichier;
  a.click();
  URL.revokeObjectURL(a.href);
}

/* ================= initialisation ================= */

function lierEvenements() {
  for (const b of $$("#onglets button"))
    b.addEventListener("click", () => montrerOnglet(b.dataset.onglet));

  $("#plan-classe").addEventListener("change", () => { etat.selectionEleve = null; chargerVuePlan(); });
  $("#suivi-classe").addEventListener("change", chargerVueSuivi);
  $("#suivi-date").addEventListener("change", () => { etat.dateSuivi = $("#suivi-date").value; });
  $("#seance-classe").addEventListener("change", chargerVueSeance);
  $("#seance-date").addEventListener("change", chargerVueSeance);
  $("#seance-jour").addEventListener("change", () => { selectionJourHeure(); chargerVueSeance(); });
  $("#seance-heure").addEventListener("change", () => { selectionJourHeure(); chargerVueSeance(); });

  for (const b of $$(".presets button"))
    b.addEventListener("click", async () => {
      const id = $("#plan-classe").value;
      if (!id) return alert("Choisissez d'abord une classe.");
      etat.plan = { classe_id: id, nom: b.dataset.preset, tables: PRESETS[b.dataset.preset]() };
      await enregistrerPlan();
      rendrePlan($("#plan-zone"), "edition");
    });
  $("#plan-enregistrer").addEventListener("click", enregistrerPlan);

  $("#voile").addEventListener("click", () => fermerModales());
  $("#modal-actions .fermer").addEventListener("click", fermerModales);
  $("#action-ajouter").addEventListener("click", () => {
    const libelle = $("#action-libelle").value.trim();
    const points = Number($("#action-points").value);
    if (!libelle) return alert("Indiquez un libellé.");
    if (!Number.isFinite(points) || points === 0) return alert("Indiquez un nombre de points non nul.");
    ajouterAction(points, libelle);
    $("#action-libelle").value = "";
    $("#action-points").value = 1;
  });
  $("#note-enregistrer").addEventListener("click", () => enregistrerNote().then(() => alert("Note enregistrée.")));

  $("#edt-importer").addEventListener("click", () => $("#edt-fichier").click());
  $("#edt-fichier").addEventListener("change", (ev) => {
    const f = ev.target.files[0];
    if (!f) return;
    const r = new FileReader();
    r.onload = async () => {
      try {
        const res = await api("/api/edt/import", { corps: JSON.parse(r.result) });
        await chargerClasses();
        await remplirChoixSeance();
        rendreEdt();
        alert(`${res.ajoutes} créneau(x) importé(s).`);
      } catch (err) { alert("Import impossible : " + err.message); }
    };
    r.readAsText(f);
    ev.target.value = "";
  });
  $("#edt-ajouter").addEventListener("click", () => {
    creneaux.push({ id: nouvelId(), jour: 0, debut: 480, duree: 55, classe_id: etat.classes[0] ? etat.classes[0].id : "", matiere: "", salle: "" });
    rendreEdt();
  });
  $("#edt-enregistrer").addEventListener("click", enregistrerEdt);

  $("#import-pronote").addEventListener("click", () => $("#import-pronote-fichier").click());
  $("#import-pronote-fichier").addEventListener("change", (ev) => {
    const f = ev.target.files[0];
    if (!f) return;
    lireFichier(f, async (donnees) => {
      try {
        await api("/api/classes/import-pronote", { corps: donnees });
        await chargerClasses();
        alert("Classe importée / mise à jour.");
      } catch (err) { alert("Import impossible : " + err.message); }
    });
    ev.target.value = "";
  });

  $("#classe-creer").addEventListener("click", async () => {
    const nom = $("#classe-nom").value.trim();
    if (!nom) return alert("Indiquez un nom de classe.");
    const eleves = $("#classe-eleves").value.split("\n").map((s) => s.trim()).filter(Boolean).map((ligne) => {
      const parties = ligne.split(/\s+/);
      const nomFamille = parties.pop().toUpperCase();
      return { id: nouvelId(), prenom: parties.join(" "), nom: nomFamille, photo: null };
    });
    await api("/api/classes", { corps: { id: nouvelId(), nom, students: eleves } });
    await chargerClasses();
    $("#classe-nom").value = "";
    $("#classe-eleves").value = "";
    alert(`Classe « ${nom} » créée avec ${eleves.length} élève(s).`);
  });

  $("#classe-supprimer").addEventListener("click", async () => {
    const id = $("#suppr-classe").value;
    const c = classeParId(id);
    if (!id || !c) return;
    if (!confirm(`Supprimer la classe ${c.nom} (élèves, plan, suivi et créneaux EDT associés) ?`)) return;
    await api("/api/classes/" + id, { methode: "DELETE" });
    await chargerClasses();
    alert("Classe supprimée.");
  });

  $("#cfg-enregistrer").addEventListener("click", enregistrerConfiguration);
}

async function init() {
  etat.config = await api("/api/config");
  remplirConfiguration();
  await chargerClasses();
  etat.dateSuivi = aujourdhui();
  $("#suivi-date").value = aujourdhui();
  $("#seance-date").value = aujourdhui();
  lierEvenements();
  await remplirChoixSeance();
  montrerOnglet("seance");
}

init().catch((err) => {
  document.body.insertAdjacentHTML("afterbegin", `<p style="background:#dc2626;color:#fff;padding:.5rem 1rem">Erreur au chargement : ${esc(err.message)}</p>`);
  console.error(err);
});
