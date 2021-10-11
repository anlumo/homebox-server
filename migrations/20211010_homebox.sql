CREATE TABLE IF NOT EXISTS locations
(
    uuid BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS containers
(
    uuid BLOB PRIMARY KEY NOT NULL,
    created DATETIME NOT NULL,
    updated DATETIME NOT NULL,
    name TEXT,
    location BLOB,
    FOREIGN KEY(location) REFERENCES locations(uuid)
);

CREATE TABLE IF NOT EXISTS items
(
    uuid BLOB PRIMARY KEY NOT NULL,
    created DATETIME NOT NULL,
    updated DATETIME NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    container BLOB NOT NULL,
    FOREIGN KEY(container) REFERENCES containers(uuid)
);
