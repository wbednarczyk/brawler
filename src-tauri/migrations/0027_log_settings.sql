INSERT OR IGNORE INTO settings (key, value, value_type)
VALUES
    ('log_level', 'info', 'string'),
    ('log_max_files', '5', 'integer'),
    ('log_max_file_bytes', '5242880', 'integer');
