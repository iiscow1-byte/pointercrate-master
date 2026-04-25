use log::{info, warn};
use sqlx::{PgConnection, Pool, Postgres, Row};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const TSV_PATH: &str = "Placing sheet - Sheet1.tsv";

const VICTOR_COLUMN_NAMES: [&str; 6] = ["SilkGMD", "HLHL", "ava", "GERG", "Emea", "Cinder"];

pub async fn run_import_if_needed(pool: &Pool<Postgres>) {
    let mut conn = match pool.acquire().await {
        Ok(c) => c,
        Err(e) => {
            warn!("[import] could not acquire db connection: {}", e);
            return;
        },
    };

    let already_populated: bool = match sqlx::query("SELECT EXISTS(SELECT 1 FROM demons LIMIT 1) AS e")
        .fetch_one(&mut *conn)
        .await
    {
        Ok(r) => r.try_get::<bool, _>("e").unwrap_or(true),
        Err(e) => {
            warn!("[import] could not check demons table: {}", e);
            return;
        },
    };

    if already_populated {
        info!("[import] demons table is not empty, skipping TSV import");
        return;
    }

    if !Path::new(TSV_PATH).exists() {
        warn!("[import] TSV file {:?} not found, skipping", TSV_PATH);
        return;
    }

    info!("[import] demons table is empty -- running one-time TSV import from {:?}", TSV_PATH);

    let raw = match fs::read_to_string(TSV_PATH) {
        Ok(s) => s,
        Err(e) => {
            warn!("[import] failed to read {}: {}", TSV_PATH, e);
            return;
        },
    };

    let rows = parse_tsv(&raw);

    // Drop the probing connection before opening a fresh transaction.
    drop(conn);

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            warn!("[import] failed to start transaction: {}", e);
            return;
        },
    };

    // audit_connection: required because auditing triggers reference the active_user temp table
    if let Err(e) = pointercrate_core::pool::audit_connection(&mut tx, 0).await {
        warn!("[import] could not set up audit connection: {}", e);
        return;
    }

    match import_rows(&rows, &mut tx).await {
        Ok(stats) => {
            if let Err(e) = tx.commit().await {
                warn!("[import] commit failed: {}", e);
                return;
            }
            info!(
                "[import] done. inserted {} demons, {} records, created {} new players",
                stats.demons, stats.records, stats.players
            );
        },
        Err(e) => {
            warn!("[import] aborting, rolling back: {}", e);
            let _ = tx.rollback().await;
        },
    }
}

#[derive(Default)]
struct Stats {
    demons: u32,
    records: u32,
    players: u32,
}

struct ParsedRow {
    name: String,
    creators: Vec<String>, // first entry is the publisher
    verifier: String,
    video: Option<String>,
    level_id: Option<i64>,
    tier: Option<i16>,
    position: i16,
    victors: [Option<String>; 6],
    requirement: i16,
}

fn parse_tsv(raw: &str) -> Vec<ParsedRow> {
    let mut out = Vec::new();
    for (line_idx, line) in raw.lines().enumerate() {
        if line_idx == 0 {
            continue; // header
        }
        if line.trim().is_empty() {
            continue;
        }

        let cols: Vec<&str> = line.split('\t').collect();
        // 0=Name, 1=Publisher/creators, 2=Verifier, 3=Video, 4=ID, 5=Tier, 6=Placement,
        // 7..13 = victors (Silk,HLHL,Ava,GERG,Emea,Cinder), 13=Requirement
        if cols.len() < 14 {
            warn!("[import] line {} has only {} columns, skipping", line_idx + 1, cols.len());
            continue;
        }

        let name = cols[0].trim().to_string();
        if name.is_empty() {
            continue;
        }

        let creators: Vec<String> = cols[1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let verifier = cols[2].trim().to_string();
        let video = {
            let v = cols[3].trim();
            if v.is_empty() { None } else { Some(v.to_string()) }
        };
        let level_id = cols[4].trim().parse::<i64>().ok().filter(|&n| n > 0);
        let tier = cols[5].trim().parse::<i16>().ok();
        let position = match cols[6].trim().parse::<i16>() {
            Ok(p) => p,
            Err(_) => {
                warn!("[import] line {} has invalid placement {:?}, skipping", line_idx + 1, cols[6]);
                continue;
            },
        };

        let mut victors: [Option<String>; 6] = Default::default();
        for i in 0..6 {
            let v = cols[7 + i].trim();
            if !v.is_empty() {
                victors[i] = Some(v.to_string());
            }
        }
        let requirement = cols[13].trim().parse::<i16>().unwrap_or(100);

        if creators.is_empty() || verifier.is_empty() {
            warn!("[import] line {} missing creators/verifier, skipping", line_idx + 1);
            continue;
        }

        out.push(ParsedRow {
            name,
            creators,
            verifier,
            video,
            level_id,
            tier,
            position,
            victors,
            requirement,
        });
    }
    out
}

/// In-memory fuzzy registry: maps hand-typed names back to canonical stored players.
struct PlayerRegistry {
    by_canonical: HashMap<String, i32>,           // canonical stored name -> id
    normalized_to_canonical: HashMap<String, String>, // normalized key -> canonical stored name
}

impl PlayerRegistry {
    fn new() -> Self {
        Self {
            by_canonical: HashMap::new(),
            normalized_to_canonical: HashMap::new(),
        }
    }

    async fn resolve(&mut self, raw: &str, conn: &mut PgConnection, stats: &mut Stats) -> sqlx::Result<i32> {
        let name = raw.trim();
        let norm = normalize(name);

        if let Some(canon) = self.normalized_to_canonical.get(&norm) {
            if let Some(id) = self.by_canonical.get(canon) {
                return Ok(*id);
            }
        }

        // Fuzzy match against already-known normalized names.
        let mut best: Option<(String, usize)> = None;
        for known in self.normalized_to_canonical.keys() {
            let d = levenshtein(&norm, known);
            let threshold = fuzzy_threshold(known);
            if d <= threshold {
                match &best {
                    None => best = Some((known.clone(), d)),
                    Some((_, bd)) if d < *bd => best = Some((known.clone(), d)),
                    _ => (),
                }
            }
        }
        if let Some((matched_norm, _)) = best {
            let canon = self.normalized_to_canonical[&matched_norm].clone();
            if let Some(id) = self.by_canonical.get(&canon) {
                if canon.to_lowercase() != name.to_lowercase() {
                    info!("[import] fuzzy-matched player {:?} -> {:?}", name, canon);
                }
                return Ok(*id);
            }
        }

        // Not cached. Check the database directly (CITEXT = case-insensitive).
        let existing = sqlx::query("SELECT id, name FROM players WHERE name = $1::CITEXT")
            .bind(name)
            .fetch_optional(&mut *conn)
            .await?;

        if let Some(row) = existing {
            let id: i32 = row.try_get("id")?;
            let stored_name: String = row.try_get("name")?;
            self.by_canonical.insert(stored_name.clone(), id);
            self.normalized_to_canonical.insert(normalize(&stored_name), stored_name);
            return Ok(id);
        }

        // Create a new player.
        let row = sqlx::query("INSERT INTO players (name) VALUES ($1) RETURNING id")
            .bind(name)
            .fetch_one(&mut *conn)
            .await?;
        let id: i32 = row.try_get("id")?;
        stats.players += 1;
        self.by_canonical.insert(name.to_string(), id);
        self.normalized_to_canonical.insert(norm, name.to_string());
        Ok(id)
    }
}

fn fuzzy_threshold(norm: &str) -> usize {
    match norm.chars().count() {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    }
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 { return n; }
    if n == 0 { return m; }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

async fn import_rows(rows: &[ParsedRow], conn: &mut PgConnection) -> sqlx::Result<Stats> {
    let mut stats = Stats::default();
    let mut registry = PlayerRegistry::new();

    // One shared submitter for the entire import.
    let submitter_row = sqlx::query(
        "INSERT INTO submitters (ip_address) VALUES (cast('127.0.0.1' as inet)) RETURNING submitter_id",
    )
    .fetch_one(&mut *conn)
    .await?;
    let submitter_id: i32 = submitter_row.try_get("submitter_id")?;

    for row in rows {
        let publisher_id = registry.resolve(&row.creators[0], &mut *conn, &mut stats).await?;
        let verifier_id = registry.resolve(&row.verifier, &mut *conn, &mut stats).await?;

        let thumbnail = row
            .video
            .as_ref()
            .and_then(|v| extract_youtube_id(v))
            .map(|id| format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", id))
            .unwrap_or_else(|| "https://i.ytimg.com/vi/zebrafishes/mqdefault.jpg".to_string());

        let demon_row = sqlx::query(
            "INSERT INTO demons (name, position, requirement, video, verifier, publisher, level_id, tier, thumbnail) \
             VALUES ($1::text, $2, $3, $4::text, $5, $6, $7, $8, $9) RETURNING id",
        )
        .bind(&row.name)
        .bind(row.position)
        .bind(row.requirement)
        .bind(row.video.as_deref())
        .bind(verifier_id)
        .bind(publisher_id)
        .bind(row.level_id)
        .bind(row.tier)
        .bind(&thumbnail)
        .fetch_one(&mut *conn)
        .await?;
        let demon_id: i32 = demon_row.try_get("id")?;
        stats.demons += 1;

        // Creators (dedup by id because (demon, creator) is a primary key).
        let mut seen_creator_ids: Vec<i32> = Vec::new();
        for c in &row.creators {
            let cid = registry.resolve(c, &mut *conn, &mut stats).await?;
            if seen_creator_ids.contains(&cid) {
                continue;
            }
            seen_creator_ids.push(cid);
            sqlx::query("INSERT INTO creators (creator, demon) VALUES ($1, $2)")
                .bind(cid)
                .bind(demon_id)
                .execute(&mut *conn)
                .await?;
        }

        // Victor-column records: each non-empty cell becomes a 100% approved record, unless the
        // player is the verifier of this demon.
        for (i, victor_cell) in row.victors.iter().enumerate() {
            let Some(cell_text) = victor_cell else { continue };

            let player_name = VICTOR_COLUMN_NAMES[i];
            let player_id = registry.resolve(player_name, &mut *conn, &mut stats).await?;

            if player_id == verifier_id {
                continue;
            }

            // Confirm the hand-typed cell text actually resembles this row's level name. This
            // guards against obvious off-by-one data-entry errors.
            let cell_norm = normalize(cell_text);
            let name_norm = normalize(&row.name);
            let tolerance = fuzzy_threshold(&name_norm).max(3);
            if levenshtein(&cell_norm, &name_norm) > tolerance {
                warn!(
                    "[import] victor cell {:?} on {:?} does not resemble level name (player {}), skipping",
                    cell_text, row.name, player_name
                );
                continue;
            }

            sqlx::query(
                "INSERT INTO records (progress, video, status_, player, submitter, demon) \
                 VALUES (100, NULL, 'APPROVED', $1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(player_id)
            .bind(submitter_id)
            .bind(demon_id)
            .execute(&mut *conn)
            .await?;
            stats.records += 1;
        }
    }

    // Recompute cached scores. These are plain SELECT-of-a-function calls.
    sqlx::query("SELECT recompute_player_scores();").execute(&mut *conn).await?;
    sqlx::query("SELECT recompute_nation_scores();").execute(&mut *conn).await?;
    sqlx::query("SELECT recompute_subdivision_scores();").execute(&mut *conn).await?;

    Ok(stats)
}

fn extract_youtube_id(url: &str) -> Option<String> {
    if let Some((_, rest)) = url.split_once("v=") {
        return Some(rest.split(['&', '#']).next().unwrap_or(rest).to_string());
    }
    if let Some((_, rest)) = url.split_once("youtu.be/") {
        return Some(rest.split(['?', '&', '#']).next().unwrap_or(rest).to_string());
    }
    None
}
