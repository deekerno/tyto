use crate::storage;
use hashbrown::HashMap;
use mysql_async::prelude::*;

pub async fn get_torrents(pool: mysql_async::Pool) -> Result<storage::TorrentRecords, mysql_async::error::Error> {
    
    let conn = pool.get_conn().await?;

    let mut torrents = storage::TorrentRecords::new();

    let result = conn.prep_exec("SELECT info_hash, complete, downloaded, incomplete, balance FROM torrents", ()).await?;

    let (_, records) = result.map_and_drop(|row| {
        let (info_hash, complete, downloaded, incomplete, balance) = mysql_async::from_row(row);
        storage::Torrent {
            info_hash,
            complete,
            downloaded,
            incomplete,
            balance
        }
    }).await?;

    for rec in records {
        torrents.insert(rec.info_hash.clone(), rec);
    }

    Ok(torrents)
}

pub async fn flush_torrents(pool: mysql_async::Pool, torrents: Vec<storage::Torrent>) -> Result<(), mysql_async::error::Error> {
    // Flushing should be accompanied by a lock on peer and torrent records
    let conn = pool.get_conn().await?;

    let params = torrents.into_iter().map(|torrent| {
        params! {
            "info_hash" => torrent.info_hash.clone(),
            "complete" => torrent.complete,
            "downloaded" => torrent.downloaded,
            "incomplete" => torrent.incomplete,
            "balance" => torrent.balance,
        }
    });

    let conn = conn.batch_exec(r"INSERT INTO torrents (info_hash, complete, downloaded, incomplete, balance)
                    VALUES (:info_hash, :complete, :downloaded, :incomplete, :balance)", params).await?;

    Ok(())
}
