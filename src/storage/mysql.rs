use crate::storage;
use mysql::prelude::*;
use mysql::*;

pub fn get_torrents(pool: Pool) -> Result<storage::TorrentRecords> {
    let mut conn = pool.get_conn()?;

    let mut torrents = storage::TorrentRecords::new();

    let selected_torrents = conn.query_map(
        "SELECT info_hash, complete, downloaded, incomplete, balance FROM torrents",
        |(info_hash, complete, downloaded, incomplete, balance)| storage::Torrent {
            info_hash,
            complete,
            downloaded,
            incomplete,
            balance,
        },
    )?;

    for sel in selected_torrents {
        torrents.insert(sel.info_hash.clone(), sel);
    }

    Ok(torrents)
}

pub fn flush_torrents(pool: Pool, torrents: Vec<storage::Torrent>) -> Result<()> {
    // Flushing should be accompanied by a lock on peer and torrent records
    let mut conn = pool.get_conn()?;

    let params = torrents.iter().map(|torrent| {
        params! {
            "info_hash" => &torrent.info_hash,
            "complete" => torrent.complete,
            "downloaded" => torrent.downloaded,
            "incomplete" => torrent.incomplete,
            "balance" => torrent.balance,
        }
    });

    conn.exec_batch(
        r"INSERT INTO torrents (info_hash, complete, downloaded, incomplete, balance)
                    VALUES (:info_hash, :complete, :downloaded, :incomplete, :balance)
                    ON DUPLICATE KEY UPDATE 
                        complete=:complete, 
                        downloaded=:downloaded, 
                        incomplete=:incomplete, 
                        balance=:balance",
        params,
    )?;

    Ok(())
}
