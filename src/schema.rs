// @generated automatically by Diesel CLI.

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

diesel::joinable!(googleaccount -> inneruser (user_id));
diesel::joinable!(inneruserdata -> inneruser (id));
diesel::joinable!(innerusersession -> inneruser (user_id));
diesel::joinable!(twitchaccount -> inneruser (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    googleaccount,
    inneruser,
    inneruserdata,
    innerusersession,
    twitchaccount,
);
