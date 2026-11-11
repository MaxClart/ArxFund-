use arxfund::library::models::Provider;
use arxfund::library::{Database, Song, SortDir, SortField};

fn make_song(title: &str) -> Song {
    Song {
        id: format!("s-{}", title),
        title: title.to_string(),
        artist: Some("Artiste".to_string()),
        album: Some("Album".to_string()),
        duration: Some(180.0),
        cover_path: None,
        provider: Provider::Local,
        provider_track_id: None,
        source_url: None,
        local_path: None,
        audio_available: true,
        is_favorite: false,
        added_at: 1000,
    }
}

#[test]
fn insert_and_list() {
    let db = Database::open_in_memory().unwrap();
    assert!(db.insert_song(&make_song("Alpha"), Some("h1")).unwrap());
    assert!(db.insert_song(&make_song("Beta"), Some("h2")).unwrap());
    let songs = db.list_songs(SortField::Title, SortDir::Asc).unwrap();
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].title, "Alpha");
}

#[test]
fn duplicate_by_hash() {
    let db = Database::open_in_memory().unwrap();
    assert!(db.insert_song(&make_song("One"), Some("samehash")).unwrap());
    assert!(!db.insert_song(&make_song("Two"), Some("samehash")).unwrap());
    assert_eq!(
        db.list_songs(SortField::Title, SortDir::Asc).unwrap().len(),
        1
    );
}

#[test]
fn search() {
    let db = Database::open_in_memory().unwrap();
    let mut s = make_song("Ocean Drive");
    s.artist = Some("Axel".to_string());
    s.album = Some("Midnight".to_string());
    db.insert_song(&s, Some("h3")).unwrap();
    assert_eq!(db.search_songs("ocean").unwrap().len(), 1);
    assert_eq!(db.search_songs("axel").unwrap().len(), 1);
    assert_eq!(db.search_songs("midnight").unwrap().len(), 1);
    assert_eq!(db.search_songs("nope").unwrap().len(), 0);
}

#[test]
fn favorites_and_delete() {
    let db = Database::open_in_memory().unwrap();
    let s = make_song("Fav");
    db.insert_song(&s, Some("hf")).unwrap();
    db.set_favorite(&s.id, true).unwrap();
    assert!(db.get_song(&s.id).unwrap().unwrap().is_favorite);
    db.delete_song(&s.id).unwrap();
    assert!(db.get_song(&s.id).unwrap().is_none());
}

#[test]
fn playlists() {
    let db = Database::open_in_memory().unwrap();
    let s1 = make_song("A");
    let s2 = make_song("B");
    db.insert_song(&s1, Some("ha")).unwrap();
    db.insert_song(&s2, Some("hb")).unwrap();
    let pl = db.create_playlist("p1", "Mix", None).unwrap();
    assert!(db.add_track_to_playlist(&pl.id, &s1.id).unwrap());
    assert!(db.add_track_to_playlist(&pl.id, &s2.id).unwrap());
    assert!(!db.add_track_to_playlist(&pl.id, &s1.id).unwrap());
    let tracks = db.playlist_tracks(&pl.id).unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title, "A");
    db.remove_track_from_playlist(&pl.id, &s1.id).unwrap();
    let tracks = db.playlist_tracks(&pl.id).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].title, "B");
    db.delete_playlist(&pl.id).unwrap();
    assert!(db.list_playlists().unwrap().is_empty());
}

#[test]
fn settings() {
    let db = Database::open_in_memory().unwrap();
    db.set_setting("k", "v").unwrap();
    assert_eq!(db.get_setting("k").unwrap(), Some("v".to_string()));
    db.set_setting("k", "v2").unwrap();
    assert_eq!(db.get_setting("k").unwrap(), Some("v2".to_string()));
    assert_eq!(db.get_setting("missing").unwrap(), None);
}

#[test]
fn count_hash() {
    let db = Database::open_in_memory().unwrap();
    db.insert_song(&make_song("X"), Some("hx")).unwrap();
    assert_eq!(db.count_hash("hx").unwrap(), 1);
    assert_eq!(db.count_hash("nope").unwrap(), 0);
}
