//! Generated views (spec 6.2, ADR 0007): the split borrow of `Connected` and
//! the views of struct-like variants.

use super::model::{Connected, Frame, Keepalive, Link, Session};

/// The fields of `Connected` without its `#[region]` fields.
///
/// Region handlers get it next to their own data, so a handler of `link` can
/// bump `frames_sent` while the dispatch still holds both regions.
pub struct ConnectedFields<'a> {
    /// `Connected::session`.
    pub session: &'a mut Session,
    /// `Connected::frames_sent`.
    pub frames_sent: &'a mut u32,
}

impl Connected {
    /// Splits the borrow: the fields, then the regions in declaration order.
    pub fn split(&mut self) -> (ConnectedFields<'_>, &mut Link, &mut Keepalive) {
        let Self {
            session,
            frames_sent,
            link,
            keepalive,
        } = self;
        (
            ConnectedFields {
                session,
                frames_sent,
            },
            link,
            keepalive,
        )
    }
}

/// View of `Conn::Connecting { attempt }`.
pub struct ConnectingMut<'a> {
    /// `attempt` of the variant.
    pub attempt: &'a mut u8,
}

/// View of `Conn::Backoff { attempt }`.
pub struct BackoffMut<'a> {
    /// `attempt` of the variant.
    pub attempt: &'a mut u8,
}

/// View of `Link::Sending { frame }`.
pub struct SendingMut<'a> {
    /// `frame` of the variant.
    pub frame: &'a mut Frame,
}
