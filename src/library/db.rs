use crate::library::models::{Playlist, Provider, Song, SortDir, SortField};
use rusqlite::{Connection, params};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS songs (
               id TEXT PRIMARY KEY,
               title TEXT NOT NULL,
               artist TEXT,
               album TEXT,
               duration REAL,
               cover_path TEXT,
               provider TEXT NOT NULL DEFAULT 'local',
               provider_track_id TEXT,
               source_url TEXT,
               local_path TEXT,
               audio_available INTEGER NOT NULL DEFAULT 1,
               file_hash TEXT UNIQUE,
               is_favorite INTEGER NOT NULL DEFAULT 0,
               added_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_songs_title ON songs(title);
             CREATE INDEX IF NOT EXISTS idx_songs_artist ON songs(artist);
             CREATE INDEX IF NOT EXISTS idx_songs_added ON songs(added_at);
             CREATE TABLE IF NOT EXISTS playlists (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               description TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS playlist_tracks (
               playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
               song_id TEXT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
               position INTEGER NOT NULL,
               added_at INTEGER NOT NULL,
               PRIMARY KEY (playlist_id, song_id)
             );
             CREATE INDEX IF NOT EXISTS idx_pt_order ON playlist_tracks(playlist_id, position);
             CREATE TABLE IF NOT EXISTS settings (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );",
        )?;
        Ok(())
    }

    // ---- settings ----
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- songs ----
    pub fn insert_song(
        &self,
        song: &Song,
        file_hash: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        // duplicate check
        if let Some(h) = file_hash {
            let exists: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM songs WHERE file_hash = ?1",
                params![h],
                |r| r.get(0),
            )?;
            if exists > 0 {
                return Ok(false);
            }
        }
        self.conn.execute(
            "INSERT INTO songs (id, title, artist, album, duration, cover_path, provider,
               provider_track_id, source_url, local_path, audio_available, file_hash, is_favorite, added_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,0,?13)",
            params![
                song.id, song.title, song.artist, song.album, song.duration, song.cover_path,
                song.provider.as_str(), song.provider_track_id, song.source_url, song.local_path,
                if song.audio_available { 1 } else { 0 }, file_hash, song.added_at
            ],
        )?;
        Ok(true)
    }

    pub fn get_song(&self, id: &str) -> Result<Option<Song>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,artist,album,duration,cover_path,provider,provider_track_id,
                    source_url,local_path,audio_available,is_favorite,added_at
             FROM songs WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_song(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_songs(&self, sort: SortField, dir: SortDir) -> Result<Vec<Song>, rusqlite::Error> {
        let col = match sort {
            SortField::Title => "title COLLATE NOCASE",
            SortField::Artist => "artist COLLATE NOCASE",
            SortField::Album => "album COLLATE NOCASE",
            SortField::Duration => "duration",
            SortField::AddedAt => "added_at",
        };
        let d = match dir {
            SortDir::Asc => "ASC",
            SortDir::Desc => "DESC",
        };
        let sql = format!(
            "SELECT id,title,artist,album,duration,cover_path,provider,provider_track_id,
                    source_url,local_path,audio_available,is_favorite,added_at
             FROM songs ORDER BY {col} {d}, title COLLATE NOCASE ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_song)?;
        rows.collect()
    }

    pub fn search_songs(&self, query: &str) -> Result<Vec<Song>, rusqlite::Error> {
        let q = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id,title,artist,album,duration,cover_path,provider,provider_track_id,
                    source_url,local_path,audio_available,is_favorite,added_at
             FROM songs
             WHERE title LIKE ?1 OR artist LIKE ?1 OR album LIKE ?1
             ORDER BY title COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map(params![q], row_to_song)?;
        rows.collect()
    }

    pub fn delete_song(&self, id: &str) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM songs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_favorite(&self, id: &str, fav: bool) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE songs SET is_favorite = ?1 WHERE id = ?2",
            params![fav as i64, id],
        )?;
        Ok(())
    }

    pub fn set_audio_available(&self, id: &str, avail: bool) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE songs SET audio_available = ?1 WHERE id = ?2",
            params![avail as i64, id],
        )?;
        Ok(())
    }

    pub fn count_hash(&self, hash: &str) -> Result<i64, rusqlite::Error> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM songs WHERE file_hash = ?1",
            params![hash],
            |r| r.get(0),
        )
    }

    pub fn count_songs(&self) -> Result<i64, rusqlite::Error> {
        self.conn
            .query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))
    }

    pub fn recent_songs(&self, limit: i64) -> Result<Vec<Song>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,artist,album,duration,cover_path,provider,provider_track_id,
                    source_url,local_path,audio_available,is_favorite,added_at
             FROM songs ORDER BY added_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_song)?;
        rows.collect()
    }

    pub fn distinct_artists(&self) -> Result<i64, rusqlite::Error> {
        self.conn.query_row(
            "SELECT COUNT(DISTINCT artist) FROM songs WHERE artist IS NOT NULL",
            [],
            |r| r.get(0),
        )
    }

    pub fn total_duration(&self) -> Result<f64, rusqlite::Error> {
        self.conn
            .query_row("SELECT COALESCE(SUM(duration),0.0) FROM songs", [], |r| {
                r.get(0)
            })
    }

    // ---- playlists ----
    pub fn create_playlist(
        &self,
        id: &str,
        name: &str,
        desc: Option<&str>,
    ) -> Result<Playlist, rusqlite::Error> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO playlists (id, name, description, created_at, updated_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, name, desc, now, now],
        )?;
        Ok(Playlist {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
            track_count: 0,
        })
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id,p.name,p.description,p.created_at,p.updated_at,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS tc
             FROM playlists p ORDER BY p.name COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                track_count: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_playlist(&self, id: &str) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_track_to_playlist(
        &self,
        playlist_id: &str,
        song_id: &str,
    ) -> Result<bool, rusqlite::Error> {
        let already: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1 AND song_id = ?2",
            params![playlist_id, song_id],
            |r| r.get(0),
        )?;
        if already > 0 {
            return Ok(false);
        }
        let max: Option<i64> = self.conn.query_row(
            "SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |r| r.get(0),
        )?;
        let pos = max.unwrap_or(-1) + 1;
        self.conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, song_id, position, added_at) VALUES (?1,?2,?3,?4)",
            params![playlist_id, song_id, pos, now_ms()],
        )?;
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now_ms(), playlist_id],
        )?;
        Ok(true)
    }

    pub fn remove_track_from_playlist(
        &self,
        playlist_id: &str,
        song_id: &str,
    ) -> Result<(), rusqlite::Error> {
        let pos: Option<i64> = self
            .conn
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 AND song_id = ?2",
                params![playlist_id, song_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(p) = pos {
            self.conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND song_id = ?2",
                params![playlist_id, song_id],
            )?;
            self.conn.execute("UPDATE playlist_tracks SET position = position - 1 WHERE playlist_id = ?1 AND position > ?2", params![playlist_id, p])?;
        }
        Ok(())
    }

    pub fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Song>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id,s.title,s.artist,s.album,s.duration,s.cover_path,s.provider,s.provider_track_id,
                    s.source_url,s.local_path,s.audio_available,s.is_favorite,s.added_at
             FROM playlist_tracks pt JOIN songs s ON s.id = pt.song_id
             WHERE pt.playlist_id = ?1 ORDER BY pt.position ASC",
        )?;
        let rows = stmt.query_map(params![playlist_id], row_to_song)?;
        rows.collect()
    }
}

fn row_to_song(row: &rusqlite::Row) -> Result<Song, rusqlite::Error> {
    Ok(Song {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        duration: row.get(4)?,
        cover_path: row.get(5)?,
        provider: Provider::from_str(&row.get::<_, String>(6)?),
        provider_track_id: row.get(7)?,
        source_url: row.get(8)?,
        local_path: row.get(9)?,
        audio_available: row.get::<_, i64>(10)? != 0,
        is_favorite: row.get::<_, i64>(11)? != 0,
        added_at: row.get(12)?,
    })
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
