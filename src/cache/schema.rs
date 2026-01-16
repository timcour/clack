// @generated automatically by Diesel CLI.

diesel::table! {
    conversations (id, workspace_id) {
        id -> Text,
        workspace_id -> Text,
        name -> Text,
        is_channel -> Nullable<Bool>,
        is_group -> Nullable<Bool>,
        is_im -> Nullable<Bool>,
        is_mpim -> Nullable<Bool>,
        is_private -> Nullable<Bool>,
        is_archived -> Bool,
        topic_value -> Nullable<Text>,
        topic_creator -> Nullable<Text>,
        topic_last_set -> Nullable<Integer>,
        purpose_value -> Nullable<Text>,
        purpose_creator -> Nullable<Text>,
        purpose_last_set -> Nullable<Integer>,
        num_members -> Nullable<Integer>,
        full_object -> Text,
        cached_at -> Timestamp,
        deleted_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    messages (conversation_id, workspace_id, ts) {
        conversation_id -> Text,
        workspace_id -> Text,
        ts -> Text,
        user_id -> Nullable<Text>,
        text -> Text,
        thread_ts -> Nullable<Text>,
        permalink -> Nullable<Text>,
        full_object -> Text,
        cached_at -> Timestamp,
        deleted_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    users (id, workspace_id) {
        id -> Text,
        workspace_id -> Text,
        name -> Text,
        real_name -> Nullable<Text>,
        deleted -> Bool,
        is_bot -> Bool,
        is_admin -> Nullable<Bool>,
        is_owner -> Nullable<Bool>,
        tz -> Nullable<Text>,
        profile_email -> Nullable<Text>,
        profile_display_name -> Nullable<Text>,
        profile_status_emoji -> Nullable<Text>,
        profile_status_text -> Nullable<Text>,
        profile_image_72 -> Nullable<Text>,
        full_object -> Text,
        cached_at -> Timestamp,
        deleted_at -> Nullable<Timestamp>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    messages,
    users,
);
