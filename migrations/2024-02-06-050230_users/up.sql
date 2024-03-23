CREATE TABLE InnerUser (
    id UUID PRIMARY KEY,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE InnerUserData (
    id UUID PRIMARY KEY,
    given_name TEXT NOT NULL,
    family_name TEXT NOT NULL,
    banner_desc TEXT NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

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