-- Table: announcements
CREATE TABLE IF NOT EXISTS "announcements" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "title" character varying NOT NULL,
  "content" text NOT NULL,
  "summary" character varying,
  "cover" character varying,
  "type" smallint,
  "status" smallint,
  "is_pinned" boolean,
  "is_global" boolean,
  "board_id" bigint,
  "published_at" timestamp with time zone,
  "expired_at" timestamp with time zone,
  "view_count" bigint,
  "created_by" bigint NOT NULL,
  "updated_by" bigint,
  FOREIGN KEY ("board_id") REFERENCES "boards"("id"),
  FOREIGN KEY ("created_by") REFERENCES "users"("id")
);

-- Table: answer_votes
CREATE TABLE IF NOT EXISTS "answer_votes" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "comment_id" bigint NOT NULL,
  "vote_type" character varying
);

-- Table: attachments
CREATE TABLE IF NOT EXISTS "attachments" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "file_id" character varying NOT NULL,
  "user_id" bigint NOT NULL,
  "plugin_id" character varying,
  "post_id" bigint,
  "reply_id" bigint,
  "original_name" character varying NOT NULL,
  "stored_name" character varying NOT NULL,
  "stored_path" character varying NOT NULL,
  "size" bigint NOT NULL,
  "file_type" character varying,
  "mime_type" character varying,
  "mime_major" character varying,
  "ext" character varying,
  "width" bigint,
  "height" bigint,
  "status" bigint,
  "upload_ip" character varying,
  "plugin_meta" json,
  "file_hash" character varying
);

-- Table: audit_logs
CREATE TABLE IF NOT EXISTS "audit_logs" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "operator_id" bigint NOT NULL,
  "operator_ip" character varying,
  "action" character varying NOT NULL,
  "target_type" character varying NOT NULL,
  "target_id" bigint NOT NULL,
  "before" text,
  "after" text,
  "reason" character varying,
  "ip" character varying,
  FOREIGN KEY ("operator_id") REFERENCES "users"("id")
);

-- Table: blocked_ips
CREATE TABLE IF NOT EXISTS "blocked_ips" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "ip" character varying NOT NULL,
  "reason" text,
  "operator_id" bigint,
  "expire_at" timestamp with time zone,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone
);

-- Table: board_bans
CREATE TABLE IF NOT EXISTS "board_bans" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "board_id" bigint NOT NULL,
  "banned_by" bigint,
  "reason" character varying,
  "expires_at" timestamp with time zone,
  FOREIGN KEY ("board_id") REFERENCES "boards"("id"),
  FOREIGN KEY ("banned_by") REFERENCES "users"("id"),
  FOREIGN KEY ("user_id") REFERENCES "users"("id")
);

-- Table: boards
CREATE TABLE IF NOT EXISTS "boards" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "name" character varying NOT NULL,
  "slug" character varying NOT NULL,
  "description" character varying,
  "icon" character varying,
  "cover" character varying,
  "parent_id" bigint,
  "sort_order" bigint,
  "view_role" character varying,
  "post_role" character varying,
  "reply_role" character varying,
  "post_count" bigint,
  "thread_count" bigint,
  "today_count" bigint,
  FOREIGN KEY ("parent_id") REFERENCES "boards"("id")
);

-- Table: bots
CREATE TABLE IF NOT EXISTS "bots" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "name" character varying NOT NULL,
  "version" character varying NOT NULL,
  "description" text,
  "summary" character varying,
  "avatar_url" character varying,
  "screenshots" json,
  "homepage_url" character varying,
  "type" character varying NOT NULL,
  "tags" json,
  "creator_id" bigint NOT NULL,
  "creator_name" character varying,
  "script_code" text,
  "script_url" character varying,
  "trigger_type" character varying NOT NULL,
  "cron_expr" character varying,
  "event_filter" character varying,
  "timeout_sec" bigint,
  "retry_times" bigint,
  "env_vars" json,
  "resource_limit" json,
  "pricing" json,
  "permissions" json,
  "enabled" boolean,
  "status" character varying,
  "exec_count" bigint,
  "last_exec_at" timestamp without time zone,
  "error_msg" text,
  "config_schema" json,
  "config_values" json
);

-- Table: casbin_rule
CREATE TABLE IF NOT EXISTS "casbin_rule" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "ptype" character varying,
  "v0" character varying,
  "v1" character varying,
  "v2" character varying,
  "v3" character varying,
  "v4" character varying,
  "v5" character varying
);

-- Table: comments
CREATE TABLE IF NOT EXISTS "comments" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "content" text NOT NULL,
  "post_id" bigint NOT NULL,
  "author_id" bigint NOT NULL,
  "parent_id" bigint,
  "like_count" bigint,
  "status" character varying,
  "is_answer" boolean,
  "is_accepted" boolean,
  "vote_count" bigint,
  FOREIGN KEY ("post_id") REFERENCES "posts"("id"),
  FOREIGN KEY ("author_id") REFERENCES "users"("id"),
  FOREIGN KEY ("parent_id") REFERENCES "comments"("id")
);

-- Table: content_audit_tasks
CREATE TABLE IF NOT EXISTS "content_audit_tasks" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "target_type" character varying NOT NULL,
  "target_id" bigint NOT NULL,
  "trigger_type" character varying NOT NULL,
  "trigger_meta" text,
  "status" character varying,
  "reviewer_id" bigint,
  "review_note" character varying,
  "reviewed_at" timestamp with time zone,
  FOREIGN KEY ("reviewer_id") REFERENCES "users"("id")
);

-- Table: favorites
CREATE TABLE IF NOT EXISTS "favorites" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "target_id" bigint NOT NULL,
  "target_type" character varying NOT NULL,
  "group_id" bigint NOT NULL,
  "status" smallint
);

-- Table: follows
CREATE TABLE IF NOT EXISTS "follows" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "follower_id" bigint NOT NULL,
  "following_id" bigint NOT NULL,
  FOREIGN KEY ("following_id") REFERENCES "users"("id"),
  FOREIGN KEY ("follower_id") REFERENCES "users"("id")
);

-- Table: ip_risk_records
CREATE TABLE IF NOT EXISTS "ip_risk_records" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "ip" character varying NOT NULL,
  "event_type" character varying NOT NULL,
  "event_detail" text,
  "expire_at" timestamp with time zone,
  "created_at" timestamp with time zone
);

-- Table: likes
CREATE TABLE IF NOT EXISTS "likes" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "post_id" bigint,
  "comment_id" bigint,
  FOREIGN KEY ("user_id") REFERENCES "users"("id"),
  FOREIGN KEY ("comment_id") REFERENCES "comments"("id"),
  FOREIGN KEY ("post_id") REFERENCES "posts"("id")
);

-- Table: moderator_applications
CREATE TABLE IF NOT EXISTS "moderator_applications" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "board_id" bigint NOT NULL,
  "reason" character varying,
  "status" character varying,
  "reviewer_id" bigint,
  "review_note" character varying,
  "req_delete_post" boolean,
  "req_pin_post" boolean,
  "req_edit_any_post" boolean,
  "req_manage_moderator" boolean,
  "req_ban_user" boolean,
  FOREIGN KEY ("reviewer_id") REFERENCES "users"("id"),
  FOREIGN KEY ("user_id") REFERENCES "users"("id"),
  FOREIGN KEY ("board_id") REFERENCES "boards"("id")
);

-- Table: moderator_logs
CREATE TABLE IF NOT EXISTS "moderator_logs" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "moderator_id" bigint NOT NULL,
  "board_id" bigint,
  "action" character varying,
  "target_type" character varying,
  "target_id" bigint,
  "reason" character varying,
  "old_value" json,
  "new_value" json,
  FOREIGN KEY ("moderator_id") REFERENCES "users"("id"),
  FOREIGN KEY ("board_id") REFERENCES "boards"("id")
);

-- Table: moderators
CREATE TABLE IF NOT EXISTS "moderators" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "board_id" bigint NOT NULL,
  "permissions" json,
  FOREIGN KEY ("user_id") REFERENCES "users"("id"),
  FOREIGN KEY ("board_id") REFERENCES "boards"("id")
);

-- Table: notifications
CREATE TABLE IF NOT EXISTS "notifications" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "sender_id" bigint,
  "type" character varying,
  "content" character varying,
  "target_id" bigint,
  "target_type" character varying,
  "is_read" boolean,
  FOREIGN KEY ("user_id") REFERENCES "users"("id"),
  FOREIGN KEY ("sender_id") REFERENCES "users"("id")
);

-- Table: plugins
CREATE TABLE IF NOT EXISTS "plugins" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "name" character varying NOT NULL,
  "slug" character varying NOT NULL,
  "version" character varying NOT NULL,
  "description" text,
  "summary" character varying,
  "icon_url" character varying,
  "screenshots" json,
  "homepage_url" character varying,
  "type" character varying NOT NULL,
  "category" character varying NOT NULL,
  "tags" json,
  "author_id" bigint,
  "author_email" character varying,
  "author_url" character varying,
  "script_url" character varying NOT NULL,
  "server_entry" character varying,
  "slots" json,
  "routes" json,
  "pricing" json,
  "compatibility" json,
  "permissions" json,
  "enabled" boolean,
  "status" character varying,
  "install_count" bigint,
  "rating" numeric,
  "config_schema" json,
  "config" json
);

-- Table: post_tags
CREATE TABLE IF NOT EXISTS "post_tags" (
  "post_id" bigint NOT NULL,
  "tag_id" bigint NOT NULL,
  PRIMARY KEY ("post_id", "tag_id"),
  FOREIGN KEY ("post_id") REFERENCES "posts"("id"),
  FOREIGN KEY ("tag_id") REFERENCES "tags"("id")
);

-- Table: posts
CREATE TABLE IF NOT EXISTS "posts" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "title" character varying NOT NULL,
  "content" text NOT NULL,
  "summary" character varying,
  "cover" character varying,
  "type" character varying,
  "post_status" character varying,
  "moderation_status" character varying,
  "author_id" bigint NOT NULL,
  "view_count" bigint,
  "like_count" bigint,
  "pin_top" boolean,
  "board_id" bigint,
  "pin_in_board" boolean,
  FOREIGN KEY ("board_id") REFERENCES "boards"("id"),
  FOREIGN KEY ("author_id") REFERENCES "users"("id")
);

-- Table: questions
CREATE TABLE IF NOT EXISTS "questions" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "post_id" bigint NOT NULL,
  "accepted_answer_id" bigint,
  "reward_score" bigint,
  "answer_count" bigint,
  "view_count" bigint,
  FOREIGN KEY ("accepted_answer_id") REFERENCES "comments"("id"),
  FOREIGN KEY ("post_id") REFERENCES "posts"("id")
);

-- Table: refresh_tokens
CREATE TABLE IF NOT EXISTS "refresh_tokens" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "token" character varying NOT NULL,
  "jti" character varying NOT NULL,
  "user_agent" character varying,
  "ip" character varying,
  "expires_at" timestamp with time zone,
  "is_used" boolean
);

-- Table: reports
CREATE TABLE IF NOT EXISTS "reports" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "reporter_id" bigint NOT NULL,
  "target_id" bigint NOT NULL,
  "target_type" character varying NOT NULL,
  "type" character varying,
  "reason" character varying NOT NULL,
  "status" character varying,
  "handler_id" bigint,
  "handle_note" character varying,
  "handle_at" timestamp with time zone,
  "content_snapshot" text,
  "reporter_ip" character varying,
  "is_anonymous" boolean,
  "priority" smallint,
  FOREIGN KEY ("reporter_id") REFERENCES "users"("id"),
  FOREIGN KEY ("handler_id") REFERENCES "users"("id")
);

-- Table: sign_ins
CREATE TABLE IF NOT EXISTS "sign_ins" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "sign_date" timestamp with time zone NOT NULL,
  "score" bigint,
  "continued" bigint
);

-- Table: tags
CREATE TABLE IF NOT EXISTS "tags" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "name" character varying NOT NULL,
  "description" character varying,
  "color" character varying,
  "post_count" bigint
);

-- Table: timeline_events
CREATE TABLE IF NOT EXISTS "timeline_events" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "actor_id" bigint NOT NULL,
  "action" character varying,
  "target_id" bigint,
  "target_type" character varying,
  "payload" json,
  "score" bigint,
  FOREIGN KEY ("actor_id") REFERENCES "users"("id"),
  FOREIGN KEY ("user_id") REFERENCES "users"("id")
);

-- Table: timeline_subscriptions
CREATE TABLE IF NOT EXISTS "timeline_subscriptions" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "subscriber_id" bigint NOT NULL,
  "target_user_id" bigint NOT NULL,
  "target_type" character varying,
  "target_id" bigint,
  "is_active" boolean
);

-- Table: topic_follows
CREATE TABLE IF NOT EXISTS "topic_follows" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "topic_id" bigint NOT NULL,
  FOREIGN KEY ("topic_id") REFERENCES "topics"("id"),
  FOREIGN KEY ("user_id") REFERENCES "users"("id")
);

-- Table: topic_posts
CREATE TABLE IF NOT EXISTS "topic_posts" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "topic_id" bigint NOT NULL,
  "post_id" bigint NOT NULL,
  "sort_order" bigint,
  "added_by" bigint,
  FOREIGN KEY ("post_id") REFERENCES "posts"("id"),
  FOREIGN KEY ("topic_id") REFERENCES "topics"("id")
);

-- Table: topics
CREATE TABLE IF NOT EXISTS "topics" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "title" character varying NOT NULL,
  "description" character varying,
  "cover" character varying,
  "creator_id" bigint NOT NULL,
  "is_public" boolean,
  "post_count" bigint,
  "follower_count" bigint,
  FOREIGN KEY ("creator_id") REFERENCES "users"("id")
);

-- Table: user_risk_records
CREATE TABLE IF NOT EXISTS "user_risk_records" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "event_type" character varying NOT NULL,
  "event_detail" text,
  "expire_at" timestamp with time zone NOT NULL,
  FOREIGN KEY ("user_id") REFERENCES "users"("id")
);

-- Table: user_timelines
CREATE TABLE IF NOT EXISTS "user_timelines" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "timeline_type" character varying,
  "last_read_at" timestamp with time zone
);

-- Table: users
CREATE TABLE IF NOT EXISTS "users" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "username" character varying NOT NULL,
  "email" character varying NOT NULL,
  "password" text NOT NULL,
  "avatar" character varying,
  "bio" character varying,
  "role" character varying,
  "score" bigint,
  "is_active" boolean,
  "is_blocked" boolean,
  "last_login" timestamp with time zone,
  "invited_by_id" bigint,
  "is_temp_password" boolean,
  "temp_password_expire" timestamp with time zone
);

-- Table: violations
CREATE TABLE IF NOT EXISTS "violations" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone,
  "deleted_at" timestamp with time zone,
  "user_id" bigint NOT NULL,
  "violation_type" smallint NOT NULL,
  "reason" character varying NOT NULL,
  "content_snapshot" text,
  "evidence_url" character varying,
  "source" smallint NOT NULL,
  "status" smallint NOT NULL,
  "operator_id" bigint,
  "punish_type" smallint,
  "punish_expire_at" timestamp with time zone,
  "appeal_status" smallint NOT NULL,
  "appeal_reason" character varying,
  "appeal_time" timestamp with time zone,
  "appeal_result" character varying
);

-- Table: votes
CREATE TABLE IF NOT EXISTS "votes" (
  "id" SERIAL NOT NULL PRIMARY KEY,
  "user_id" bigint,
  "comment_id" bigint,
  "value" bigint NOT NULL,
  "created_at" timestamp with time zone,
  "updated_at" timestamp with time zone
);
