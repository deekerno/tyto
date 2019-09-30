#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Started,
    Stopped,
    Completed,
    None,
}

pub fn string_to_event(s: String) -> Event {
    match s.as_ref() {
        "started" => Event::Started,
        "stopped" => Event::Stopped,
        "completed" => Event::Completed,
        "" => Event::None,
        _ => Event::None,
        // MAYBE:
        // This should probably return an error such as "Error:
        // Malformed Request" along with the PeerID of the client,
        // and ignore the request from the client.
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
        assert_eq!(string_to_event(s), Event::Started);
    }

    #[test]
    fn event_string_to_event_garbage() {
        let s = "garbage".to_string();
        assert_eq!(string_to_event(s), Event::None);
    }

    #[test]
    fn event_event_to_string() {
        let event = Event::Completed;
        assert_eq!(event_to_string(event), "completed");
    }
}
