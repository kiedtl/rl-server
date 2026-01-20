CREATE TABLE IF NOT EXISTS "Scores" (
	id		INTEGER 	NOT NULL,
	date		INTEGER 	NOT NULL,
	player		INTEGER 	NOT NULL,
	result		INTEGER 	NOT NULL,
	morgue		TEXT		NOT NULL,
	end_level	INTEGER,
	slain_foes	INTEGER,
	stabbed_foes	INTEGER,
	
	PRIMARY KEY("id"),
	FOREIGN KEY(player) REFERENCES Players(id)
) STRICT;

CREATE TABLE IF NOT EXISTS "Players" (
	"id"		INTEGER 	NOT NULL,
	"name"		TEXT 		NOT NULL UNIQUE,
	PRIMARY KEY("id")
) STRICT;
