// This is a list of errors that are available to send back to the client.
#[derive(Debug)]
pub enum ClientError {
    MalformedAnnounce,
    MalformedScrape,
    NotCompact,
    ResourceDoesNotExist,
    UnapprovedClient,
    UnapprovedTorrent,
}

// This is a list of errors that are internal to the tracker,
// and may possibly show up in the logs.
pub enum InternalError {
    ConfigFileOpen,
    ConfigFileRead,
    ConfigParse,
    ConfigReload,
    StorageTorrentFetchNew,
    StorageTorrentFlush,
    StorageTorrentLoad,
}

impl ClientError {
    pub fn text(&self) -> String {
        match *self {
            ClientError::MalformedAnnounce => "Malformed announce request".to_string(),
            ClientError::MalformedScrape => "Malformed scrape request".to_string(),
            ClientError::NotCompact => "Announces must be in compact format".to_string(),
            ClientError::ResourceDoesNotExist => "Resource does not exist".to_string(),
            ClientError::UnapprovedClient => "Unapproved client".to_string(),
            ClientError::UnapprovedTorrent => "Unapproved torrent".to_string(),
        }
    }
}
