CREATE TABLE IF NOT EXISTS torrents (
        info_hash VARCHAR(50) NOT NULL UNIQUE,
        complete INT NOT NULL,
        downloaded INT NOT NULL,
        incomplete INT NOT NULL,
        balance BIGINT NOT NULL,
        PRIMARY KEY (info_hash)
) ENGINE = InnoDB;
