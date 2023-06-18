--
-- Metadata table SQL schema.
--
create table if not exists "metadata" (
  -- Node ID.
  "node"         text      not null,
  -- UNIX time at which this entry was added or refreshed.
  "time"         integer   not null,
  primary key ("time", "node")
);
