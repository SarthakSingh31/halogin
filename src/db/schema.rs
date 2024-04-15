// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    chatcontractoffer (id) {
        id -> Int8,
        message_id -> Int8,
        offered_payout -> Money,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    chatcontractofferupdate (id) {
        id -> Int8,
        message_id -> Int8,
        offer_id -> Int8,
        update_kind -> Contractofferstatus,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    chatlastseen (room_id, user_id) {
        room_id -> Uuid,
        user_id -> Uuid,
        last_message_seen_id -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    chatmessage (id) {
        id -> Int8,
        room_id -> Uuid,
        from_user_id -> Uuid,
        content -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    chatroom (id) {
        id -> Uuid,
        company_id -> Uuid,
        user_id -> Uuid,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    company (id) {
        id -> Uuid,
        full_name -> Text,
        banner_desc -> Text,
        logo_url -> Text,
        industry -> Array<Nullable<Text>>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    companyuser (company_id, user_id) {
        company_id -> Uuid,
        user_id -> Uuid,
        is_admin -> Bool,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    creatordata (user_id) {
        user_id -> Uuid,
        given_name -> Text,
        family_name -> Text,
        pronouns -> Text,
        profile_desc -> Text,
        content_desc -> Text,
        audience_desc -> Text,
        pfp_path -> Nullable<Text>,
        embedding -> Vector,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    googleaccount (sub) {
        sub -> Text,
        email -> Text,
        access_token -> Text,
        expires_at -> Timestamp,
        refresh_token -> Text,
        user_id -> Uuid,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    inneruser (id) {
        id -> Uuid,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    innerusersession (token) {
        token -> Text,
        expires_at -> Timestamp,
        user_id -> Uuid,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    sessionfcmtoken (token) {
        token -> Text,
        session_token -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::super::sql_types::*;

    twitchaccount (id) {
        id -> Text,
        access_token -> Text,
        expires_at -> Timestamp,
        refresh_token -> Text,
        user_id -> Uuid,
    }
}

diesel::joinable!(chatcontractoffer -> chatmessage (message_id));
diesel::joinable!(chatcontractofferupdate -> chatcontractoffer (offer_id));
diesel::joinable!(chatcontractofferupdate -> chatmessage (message_id));
diesel::joinable!(chatlastseen -> chatmessage (last_message_seen_id));
diesel::joinable!(chatlastseen -> chatroom (room_id));
diesel::joinable!(chatlastseen -> inneruser (user_id));
diesel::joinable!(chatmessage -> chatroom (room_id));
diesel::joinable!(chatmessage -> inneruser (from_user_id));
diesel::joinable!(chatroom -> company (company_id));
diesel::joinable!(chatroom -> inneruser (user_id));
diesel::joinable!(companyuser -> company (company_id));
diesel::joinable!(companyuser -> inneruser (user_id));
diesel::joinable!(creatordata -> inneruser (user_id));
diesel::joinable!(googleaccount -> inneruser (user_id));
diesel::joinable!(innerusersession -> inneruser (user_id));
diesel::joinable!(sessionfcmtoken -> innerusersession (session_token));
diesel::joinable!(twitchaccount -> inneruser (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    chatcontractoffer,
    chatcontractofferupdate,
    chatlastseen,
    chatmessage,
    chatroom,
    company,
    companyuser,
    creatordata,
    googleaccount,
    inneruser,
    innerusersession,
    sessionfcmtoken,
    twitchaccount,
);
