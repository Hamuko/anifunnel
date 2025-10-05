CREATE TABLE authentication (
    id INTEGER PRIMARY KEY,
    token VARCHAR(255) NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    username VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expiry TIMESTAMP NOT NULL
);

CREATE TABLE overrides (
    id INTEGER PRIMARY KEY,
    title VARCHAR(255) UNIQUE,
    episode_offset INTEGER
);
