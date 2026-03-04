CREATE TABLE IF NOT EXISTS domains (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'deleted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    local_part TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('super_admin', 'domain_admin', 'user')),
    quota_bytes BIGINT NOT NULL DEFAULT 0,
    used_bytes BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'deleted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ,
    UNIQUE(domain_id, local_part)
);

CREATE TABLE IF NOT EXISTS mailboxes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_id UUID REFERENCES mailboxes(id) ON DELETE SET NULL,
    uid_validity INTEGER NOT NULL,
    next_uid INTEGER NOT NULL DEFAULT 1,
    message_count BIGINT NOT NULL DEFAULT 0,
    unseen_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, name, parent_id)
);

CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY,
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    blob_id UUID NOT NULL,
    size_bytes BIGINT NOT NULL,
    flags TEXT[] NOT NULL DEFAULT '{}',
    subject TEXT,
    from_address TEXT NOT NULL,
    to_addresses TEXT[] NOT NULL DEFAULT '{}',
    cc_addresses TEXT[] NOT NULL DEFAULT '{}',
    bcc_addresses TEXT[] NOT NULL DEFAULT '{}',
    reply_to TEXT,
    message_id TEXT,
    in_reply_to TEXT,
    references TEXT[] NOT NULL DEFAULT '{}',
    sent_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(mailbox_id, uid)
);

CREATE INDEX IF NOT EXISTS idx_domains_name ON domains(name);
CREATE INDEX IF NOT EXISTS idx_users_domain_id ON users(domain_id);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_mailboxes_user_id ON mailboxes(user_id);
CREATE INDEX IF NOT EXISTS idx_mailboxes_parent_id ON mailboxes(parent_id);
CREATE INDEX IF NOT EXISTS idx_messages_mailbox_received_at ON messages(mailbox_id, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_user_received_at ON messages(user_id, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);
