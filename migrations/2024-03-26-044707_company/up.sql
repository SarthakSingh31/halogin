CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE Company (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    full_name TEXT NOT NULL,
    banner_desc TEXT NOT NULL,
    logo_url TEXT NOT NULL,
    embedding VECTOR(1536) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX company_embedding ON Company USING hnsw (embedding vector_ip_ops) WITH (m = 40, ef_construction = 160);

CREATE TABLE CompanyUser (
    company_id UUID NOT NULL,
    user_id UUID NOT NULL,
    is_admin BOOLEAN NOT NULL,
    CONSTRAINT fk_company FOREIGN KEY (company_id) REFERENCES Company(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE,
    CONSTRAINT pk_company_user PRIMARY KEY (company_id, user_id)
);

CREATE TABLE CompanyUserProfile (
    user_id UUID PRIMARY KEY,
    given_name TEXT NOT NULL,
    family_name TEXT NOT NULL,
    pronouns TEXT NOT NULL,
    pfp_path TEXT NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE CompanyUserInvitation (
    invited_google_email TEXT NOT NULL,
    company_id UUID NOT NULL,
    will_be_given_admin BOOLEAN NOT NULL,
    from_user_id UUID NOT NULL,
    CONSTRAINT fk_company FOREIGN KEY (company_id) REFERENCES Company(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (from_user_id) REFERENCES InnerUser(id) ON DELETE CASCADE,
    CONSTRAINT pk_invited_user PRIMARY KEY (invited_google_email, company_id)
);

CREATE TABLE ChatRoom (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL,
    user_id UUID NOT NULL,
    CONSTRAINT fk_company FOREIGN KEY (company_id) REFERENCES Company(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE SessionFcmToken (
    token TEXT PRIMARY KEY,
    session_token TEXT NOT NULL,
    CONSTRAINT fk_session FOREIGN KEY (session_token) REFERENCES InnerUserSession(token) ON DELETE CASCADE
);

CREATE TABLE ChatMessage (
    id BIGSERIAL PRIMARY KEY,
    room_id UUID NOT NULL,
    from_user_id UUID NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_room FOREIGN KEY (room_id) REFERENCES ChatRoom(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (from_user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE ChatLastSeen (
    room_id UUID NOT NULL,
    user_id UUID NOT NULL,
    last_message_seen_id BIGSERIAL NOT NULL,
    CONSTRAINT fk_room FOREIGN KEY (room_id) REFERENCES ChatRoom(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE,
    CONSTRAINT fk_message FOREIGN KEY (last_message_seen_id) REFERENCES ChatMessage(id) ON DELETE CASCADE,
    CONSTRAINT pk_room_user PRIMARY KEY (room_id, user_id)
);

CREATE TABLE ChatContractOffer (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGSERIAL NOT NULL,
    offered_payout MONEY NOT NULL,
    CONSTRAINT fk_message FOREIGN KEY (message_id) REFERENCES ChatMessage(id) ON DELETE CASCADE
);

CREATE TYPE ContractOfferStatus AS ENUM (
    'AcceptedByCreator',
    'WithdrawnByCompany',
    'CancelledByCreator',
    'FinishedByCreator',
    'ApprovedByCompany'
);

CREATE TABLE ChatContractOfferUpdate (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGSERIAL NOT NULL,
    offer_id BIGSERIAL NOT NULL,
    update_kind ContractOfferStatus NOT NULL,
    CONSTRAINT fk_message FOREIGN KEY (message_id) REFERENCES ChatMessage(id) ON DELETE CASCADE,
    CONSTRAINT fk_offer FOREIGN KEY (offer_id) REFERENCES ChatContractOffer(id) ON DELETE CASCADE
);

CREATE FUNCTION check_contract_offer_update() RETURNS trigger LANGUAGE plpgsql STABLE AS
$$
DECLARE
    correct_transition BOOLEAN;
BEGIN
    SELECT CASE (SELECT update_kind FROM ChatContractOfferUpdate WHERE offer_id = old.offer_id ORDER BY id DESC LIMIT 1)
        WHEN 'AcceptedByCreator' THEN
            CASE new.update_kind
                WHEN 'CancelledByCreator' THEN 'true'::BOOLEAN
                WHEN 'FinishedByCreator' THEN 'true'::BOOLEAN
                ELSE 'false'::BOOLEAN
            END
        WHEN 'WithdrawnByCompany' THEN 'false'::BOOLEAN
        WHEN 'CancelledByCreator' THEN 'false'::BOOLEAN
        WHEN 'FinishedByCreator' THEN
            CASE new.update_kind
                WHEN 'ApprovedByCompany' THEN 'true'::BOOLEAN
                ELSE 'false'::BOOLEAN
            END
        WHEN 'ApprovedByCompany' THEN 'false'::BOOLEAN
        ELSE 'true'::BOOLEAN
    END AS correct_transition;

    IF correct_transition THEN
        RETURN new;
    ELSE
        RAISE EXCEPTION 'Cannot do this state transition';
    END IF;
END
$$;

CREATE TRIGGER contract_offer_update_checker BEFORE INSERT ON ChatContractOfferUpdate FOR EACH ROW EXECUTE PROCEDURE check_contract_offer_update();