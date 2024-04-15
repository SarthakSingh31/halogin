CREATE EXTENSION vector;

SET hnsw.ef_search = 1000;

SET maintenance_work_mem = '8GB';

SET max_parallel_maintenance_workers = 7;

CREATE TABLE InnerUser (
    id UUID PRIMARY KEY,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE CreatorData (
    user_id UUID PRIMARY KEY,
    given_name TEXT NOT NULL,
    family_name TEXT NOT NULL,
    pronouns TEXT NOT NULL,
    profile_desc TEXT NOT NULL,
    content_desc TEXT NOT NULL,
    audience_desc TEXT NOT NULL,
    pfp_path TEXT,
    embedding VECTOR(1024) NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE INDEX creator_data_embedding ON CreatorData USING hnsw (embedding vector_ip_ops) WITH (m = 40, ef_construction = 160);

CREATE TABLE InnerUserSession (
    token TEXT PRIMARY KEY,
    expires_at TIMESTAMP NOT NULL,
    user_id UUID NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE TwitchAccount (
    id TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    refresh_token TEXT NOT NULL,
    user_id UUID NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE GoogleAccount (
    sub TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    access_token TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    refresh_token TEXT NOT NULL,
    user_id UUID NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);