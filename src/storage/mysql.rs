use crate::storage;
use hashbrown::HashMap;
use sqlx::MySqlPool;

pub async fn get_torrents(pool: &mut MySqlPool) -> Result<storage::TorrentRecords, std::io::Error> {
    
    let records = sqlx::query!(
        r#"
SELECT ID, info_hash, complete, downloaded, incomplete, balance
FROM torrents
ORDER BY id
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut torrents = storage::TorrentRecords::new();

    for rec in records {
        torrents.insert(
            rec.info_hash,
            storage::Torrent::new(
                rec.info_hash,
                rec.complete,
                rec.downloaded,
                rec.incomplete,
                rec.balance
            )
        );
    }

    Ok(torrents)
}

pub async fn flush_torrents(mut pool: &MySqlPool, torrents: Vec<storage::Torrent>) -> Result<(), std::io::Error> {
    // Flushing should be accompanied by a lock on peer and torrent records

    // TODO: I don't think that sqlx has a nice way of combining iterators
    // and inserts, so this will just have to do for the time being.
    for torrent in torrents {
        let _rec = sqlx::query!(
            r#"
INSERT INTO torrents ( info_hash, complete, downloaded, incomplete, balance )
VALUES ( $1, $2, $3, $4, $5 )
ON DUPLICATE KEY UPDATE complete=VALUES($2), downloaded=VALUES($3), incomplete=VALUES($4), balance=VALUES($5)
        "#,
        torrent.info_hash,
        torrent.complete,
        torrent.downloaded,
        torrent.incomplete,
        torrent.balance
        )
        .execute(&mut pool)
        .await?;
    }

    Ok(())
}
