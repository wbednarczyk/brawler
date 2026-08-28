-- Retire the docking engine (ADR 0108): the freeform cockpit and dockview are
-- removed, and `cockpit_layouts` held only auto-generated `dashboard:*` rows
-- (zero user-created named views). A pre-migration snapshot is automatic.

DROP TABLE IF EXISTS cockpit_layouts;
