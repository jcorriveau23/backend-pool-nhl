#!/bin/bash
# Runs once, on first init of an empty mongo data volume (via
# /docker-entrypoint-initdb.d). Restores the BSON snapshot dumped from the
# host's hockeypool DB so the container starts pre-loaded with real data.
# On later `up`s the volume is non-empty, so the entrypoint skips this.
set -e
echo "[seed] restoring hockeypool from archive ..."
mongorestore --drop --gzip --archive=/seed/hockeypool.archive.gz

echo "[seed] creating indexes ..."
# players: the get_players read path filters on position and sorts by
# salary_cap (default) or points. These compound indexes follow the
# Equality->Sort rule so the query is served by an index scan instead of a
# full COLLSCAN + in-memory sort. The trailing _id matches the query's
# tiebreaker so the sort is fully covered.
mongosh --quiet hockeypool --eval '
  db.players.createIndex({ salary_cap: -1, _id: 1 });
  db.players.createIndex({ points: -1, _id: 1 });
  db.players.createIndex({ position: 1, salary_cap: -1, _id: 1 });
  db.players.createIndex({ position: 1, points: -1, _id: 1 });
  db.pools.createIndex({ season: 1 });
'
echo "[seed] done."
