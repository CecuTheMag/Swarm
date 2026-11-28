-- Features the platform admin switches on and off at run time (spec 0020), without a
-- deploy: the server refuses what is off, and the console hides it.
CREATE TABLE feature_flags (
    key         text PRIMARY KEY,
    enabled     boolean     NOT NULL,
    description text        NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    updated_by  uuid        REFERENCES accounts (id) ON DELETE SET NULL
);

INSERT INTO feature_flags (key, enabled, description) VALUES
    ('ai_jobs', true, 'Задачи „Текст с ИИ“: езикови модели в браузъра'),
    ('chaos_panel', true, 'Тест на устойчивостта: изключване на устройства и добавяне на измамници'),
    ('public_programs', true, 'Одобрени програми на мрежите в общата мрежа');
