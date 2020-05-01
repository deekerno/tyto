use crate::errors::ClientError;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Started,
    Stopped,
    Completed,
    None,
}

pub fn string_to_event(s: String) -> Result<Event, ClientError> {
    match s.as_ref() {
        "started" => Ok(Event::Started),
        "stopped" => Ok(Event::Stopped),
        "completed" => Ok(Event::Completed),
        "" => Ok(Event::None),
        _ => Err(ClientError::MalformedAnnounce),
    }
}

pub fn event_to_string(event: Event) -> &'static str {
    match event {
        Event::Started => "started",
        Event::Stopped => "stopped",
        Event::Completed => "completed",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::{event_to_string, string_to_event, Event};

    #[test]
    fn event_string_to_event_good() {
        let s = "started".to_string();
        assert_eq!(string_to_event(s).unwrap(), Event::Started);
    }

    #[test]
    fn event_string_to_event_garbage() {
        let s = "garbage".to_string();
        assert!(
            string_to_event(s).is_err(),
            "String 'garbage' should result in error"
        );
    }

    #[test]
    fn event_event_to_string() {
        let event = Event::Completed;
        assert_eq!(event_to_string(event), "completed");
    }
}
