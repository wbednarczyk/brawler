-- ADR 0110 (#462): the entitlement module is retired; the table stays
-- (migrations are append-only), its rows go — no reader remains.

DELETE FROM license_metadata;
