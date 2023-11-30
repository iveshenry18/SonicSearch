CREATE TABLE IF NOT EXISTS audio_file(
    file_hash text PRIMARY KEY NOT NULL,
    file_path text NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_file_segment(
    rowid INTEGER PRIMARY KEY ASC,
    file_hash text NOT NULL,
    starting_timestamp real NOT NULL,
    embedding BLOB NOT NULL,
    FOREIGN KEY (file_hash) REFERENCES audio_file(file_hash),
    UNIQUE(file_hash, starting_timestamp)
);
