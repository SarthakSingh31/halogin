CREATE TABLE Company (
    id UUID PRIMARY KEY,
    full_name TEXT NOT NULL,
    banner_desc TEXT NOT NULL,
    logo_url TEXT NOT NULL,
    industry TEXT [] NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE CompanyUser (
    company_id UUID NOT NULL,
    user_id UUID NOT NULL,
    is_admin BOOLEAN NOT NULL,
    CONSTRAINT fk_company FOREIGN KEY (company_id) REFERENCES Company(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE,
    CONSTRAINT pk_company_user PRIMARY KEY (company_id, user_id)
);

CREATE TABLE ChatRoom (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL,
    user_id UUID NOT NULL,
    CONSTRAINT fk_company FOREIGN KEY (company_id) REFERENCES Company(id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
);

CREATE TABLE UserFcmToken (
    token TEXT PRIMARY KEY,
    user_id UUID NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES InnerUser(id) ON DELETE CASCADE
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

CREATE TYPE ContractStatus AS ENUM (
    'AcceptedByCreator',
    'WithdrawnByCompany',
    'CancelledByCreator',
    'FinishedByCreator',
    'ApprovedByCompany'
);

CREATE TABLE ChatContractUpdate (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGSERIAL NOT NULL,
    offer_id BIGSERIAL NOT NULL,
    update_kind ContractStatus NOT NULL,
    CONSTRAINT fk_message FOREIGN KEY (message_id) REFERENCES ChatMessage(id) ON DELETE CASCADE,
    CONSTRAINT fk_offer FOREIGN KEY (offer_id) REFERENCES ChatContractOffer(id) ON DELETE CASCADE
);

CREATE FUNCTION check_contract_update() RETURNS trigger LANGUAGE plpgsql STABLE AS
$$
DECLARE
    correct_transition BOOLEAN;
BEGIN
    SELECT CASE (SELECT update_kind FROM ChatContractUpdate WHERE offer_id = old.offer_id ORDER BY id DESC LIMIT 1)
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

CREATE TRIGGER contract_update_checker BEFORE INSERT ON ChatContractUpdate FOR EACH ROW EXECUTE PROCEDURE check_contract_update();