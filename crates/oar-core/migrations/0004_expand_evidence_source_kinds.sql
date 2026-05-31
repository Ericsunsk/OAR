-- Keep backend evidence source kinds aligned with the client read model.

ALTER TABLE evidence_items
DROP CONSTRAINT IF EXISTS evidence_items_source_kind_check;

ALTER TABLE evidence_items
ADD CONSTRAINT evidence_items_source_kind_check CHECK (
    source_kind IN (
        'okr_progress',
        'lark_minutes',
        'lark_doc',
        'lark_task',
        'lark_calendar',
        'lark_im',
        'manual_review_note',
        'audit_event'
    )
);
