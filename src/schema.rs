// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "contractstatus"))]
    pub struct Contractstatus;
}

diesel::table! {
    chatcontractoffer (id) {
        id -> Int8,
        message_id -> Int8,
        offered_payout -> Money,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Contractstatus;

    chatcontractupdate (id) {
        id -> Int8,
        message_id -> Int8,
        offer_id -> Int8,
        update_kind -> Contractstatus,
    }
}

diesel::table! {
    chatlastseen (room_id, user_id) {
        room_id -> Uuid,
        user_id -> Uuid,
        last_message_seen_id -> Int8,
    }
}

diesel::table! {
    chatmessage (id) {
        id -> Int8,
        room_id -> Uuid,
        from_user_id -> Uuid,
        content -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    chatroom (id) {
        id -> Uuid,
        company_id -> Uuid,
        user_id -> Uuid,
    }
}

diesel::table! {
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
    companyuser (company_id, user_id) {
        company_id -> Uuid,
        user_id -> Uuid,
        is_admin -> Bool,
    }
}

diesel::table! {
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
    inneruser (id) {
        id -> Uuid,
        created_at -> Timestamp,
    }
}

diesel::table! {
    inneruserdata (id) {
        id -> Uuid,
        given_name -> Text,
        family_name -> Text,
        banner_desc -> Text,
    }
}

diesel::table! {
    innerusersession (token) {
        token -> Text,
        expires_at -> Timestamp,
        user_id -> Uuid,
    }
}

diesel::table! {
    twitchaccount (id) {
        id -> Text,
        access_token -> Text,
        expires_at -> Timestamp,
        refresh_token -> Text,
        user_id -> Uuid,
    }
}

diesel::joinable!(chatcontractoffer -> chatmessage (message_id));
diesel::joinable!(chatcontractupdate -> chatcontractoffer (offer_id));
diesel::joinable!(chatcontractupdate -> chatmessage (message_id));
diesel::joinable!(chatlastseen -> chatmessage (last_message_seen_id));
diesel::joinable!(chatlastseen -> chatroom (room_id));
diesel::joinable!(chatlastseen -> inneruser (user_id));
diesel::joinable!(chatmessage -> chatroom (room_id));
diesel::joinable!(chatmessage -> inneruser (from_user_id));
diesel::joinable!(chatroom -> company (company_id));
diesel::joinable!(chatroom -> inneruser (user_id));
diesel::joinable!(companyuser -> company (company_id));
diesel::joinable!(companyuser -> inneruser (user_id));
diesel::joinable!(googleaccount -> inneruser (user_id));
diesel::joinable!(inneruserdata -> inneruser (id));
diesel::joinable!(innerusersession -> inneruser (user_id));
diesel::joinable!(twitchaccount -> inneruser (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    chatcontractoffer,
    chatcontractupdate,
    chatlastseen,
    chatmessage,
    chatroom,
    company,
    companyuser,
    googleaccount,
    inneruser,
    inneruserdata,
    innerusersession,
    twitchaccount,
);
