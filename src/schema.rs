// @generated automatically by Diesel CLI.

diesel::table! {
    googlesession (sub) {
        sub -> Text,
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
    twitchsession (id) {
        id -> Text,
        access_token -> Text,
        expires_at -> Timestamp,
        refresh_token -> Text,
        user_id -> Uuid,
    }
}

diesel::joinable!(googlesession -> inneruser (user_id));
diesel::joinable!(inneruserdata -> inneruser (id));
diesel::joinable!(innerusersession -> inneruser (user_id));
diesel::joinable!(twitchsession -> inneruser (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    googlesession,
    inneruser,
    inneruserdata,
    innerusersession,
    twitchsession,
);
