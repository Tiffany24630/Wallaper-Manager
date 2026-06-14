use rusqlite::Connection;

pub fn run_migrations() {
    let conn = Connection::open("wallpaper_manager.db").expect("db");

    let _ = conn.execute(
        "
        ALTER TABLE wallpapers
        ADD COLUMN hash TEXT
        ",
        []
    );

    let _ = conn.execute(
        "
        ALTER TABLE wallpapers
        ADD COLUMN width INTEGER
        ",
        []
    );

    let _ = conn.execute(
        "
        ALTER TABLE wallpapers
        ADD COLUMN height INTEGER
        ",
        []
    );

    let _ = conn.execute(
        "
        ALTER TABLE wallpapers
        ADD COLUMN size_bytes INTEGER
        ",
        []
    );

    let _ = conn.execute(
        "
        ALTER TABLE wallpapers
        ADD COLUMN file_type TEXT
        ",
        []
    );
}