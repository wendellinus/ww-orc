ALTER TABLE images
ADD COLUMN pixel_sha256 TEXT;

CREATE INDEX images_workspace_pixel_sha256_idx
    ON images(workspace_id, pixel_sha256)
    WHERE pixel_sha256 IS NOT NULL;
