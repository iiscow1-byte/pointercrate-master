-- One-time fix for position gaps left behind by earlier delete code paths
-- that didn't shift remaining demons. Renumbers all demons to the
-- consecutive sequence 1..=N preserving existing relative ordering.
UPDATE demons
SET position = renumbered.new_position
FROM (
    SELECT id, ROW_NUMBER() OVER (ORDER BY position) :: SMALLINT AS new_position
    FROM demons
    WHERE position > 0
) AS renumbered
WHERE demons.id = renumbered.id
  AND demons.position <> renumbered.new_position;
