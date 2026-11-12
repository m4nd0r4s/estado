//! Handlers as `#[handlers]` would rewrite them.
//!
//! Each function shows the source the user writes in a comment, followed by
//! the rewritten body: every `goto!` became `Next::Goto(<Edges>::<Variant>(..))`,
//! one variant per `goto!` occurrence, and the return type became
//! `Next<impl Edge<Player> + use<'event>>`.
//!
//! The `use<'event>` bound (precise capturing, stable since 1.82) is what lets
//! the generated dispatch apply the edge after the handler returns: without
//! it, edition 2024 captures every lifetime in scope, including the borrow of
//! the state data, and the take in spec 5.3 step 4 would not compile.

use crate::core::{Context, Edge, Next};
use crate::edges::{ActiveEdges, PlayingEdges};
use crate::model::{Active, ActiveFields, Event, Player, SeekError};

impl Player {
    // #[on(Playing)]
    // fn playing(position: &mut u32, up: ActiveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
    //     match event {
    //         Event::Pause => goto!(Paused { position: *position }),
    //         Event::Error(message) => goto!(Failed, |old: Exiting![Active]| Player::Failed {
    //             reason: message.clone(), buffer: old.buffer }),
    //         Event::Tick => {
    //             *position += 1;
    //             if *position > 3 { return goto!(Idle); }
    //             stay!()
    //         }
    //         Event::Play(track) => match *track {
    //             0 => goto!(Idle),
    //             same if same == *up.track => stay!(),
    //             _ => goto!(Failed, |old: Exiting![Active]| Player::Failed {
    //                 reason: "track change while playing".into(), buffer: old.buffer }),
    //         },
    //         Event::MaybeStop(code) => code
    //             .map(|code| goto!(Failed, move |old: Exiting![Active]| Player::Failed {
    //                 reason: format!("stopped with {code}"), buffer: old.buffer }))
    //             .unwrap_or_else(|| stay!()),
    //         _ => bubble!(),
    //     }
    // }
    pub fn playing<'event>(
        position: &mut u32,
        up: ActiveFields<'_>,
        context: &mut Context,
        event: &'event Event,
    ) -> Next<impl Edge<Player> + use<'event>> {
        context.note("handler Playing");
        match event {
            Event::Pause => Next::Goto(PlayingEdges::ToPaused {
                position: *position,
            }),
            // The constructor borrows `message` from the event: allowed by
            // `use<'event>`, the event outlives the whole microstep.
            Event::Error(message) => {
                Next::Goto(PlayingEdges::ToFailed0(|old: Active| Player::Failed {
                    reason: message.clone(),
                    buffer: old.buffer,
                }))
            }
            Event::Tick => {
                *position += 1;
                if *position > 3 {
                    return Next::Goto(PlayingEdges::ToIdle);
                }
                Next::Stay
            }
            Event::Play(track) => match *track {
                0 => Next::Goto(PlayingEdges::ToIdle),
                same if same == *up.track => Next::Stay,
                _ => Next::Goto(PlayingEdges::ToFailed1(|old: Active| Player::Failed {
                    reason: "track change while playing".into(),
                    buffer: old.buffer,
                })),
            },
            Event::MaybeStop(code) => code
                .map(|code| {
                    Next::Goto(PlayingEdges::ToFailed2(move |old: Active| Player::Failed {
                        reason: format!("stopped with {code}"),
                        buffer: old.buffer,
                    }))
                })
                .unwrap_or_else(|| Next::Stay),
            _ => Next::Bubble,
        }
    }

    // #[on(Active)]
    // fn active(up: ActiveFields<'_>, event: &Event) -> Result<Next<Self>, SeekError> {
    //     match event {
    //         Event::Seek(text) => {
    //             let position: u32 = text.parse()?;
    //             Ok(goto!(Playing { position }))
    //         }
    //         Event::Resume => Ok(goto!(Playing { position: 0 })),
    //         Event::Play(track) => {
    //             let mut attempts = 0;
    //             let next = loop {
    //                 attempts += 1;
    //                 if attempts == 2 { break goto!(Active, external); }
    //             };
    //             *up.track = *track;
    //             Ok(next)
    //         }
    //         _ => Ok(unhandled!()),
    //     }
    // }
    pub fn active<'event>(
        up: ActiveFields<'_>,
        context: &mut Context,
        event: &'event Event,
    ) -> Result<Next<impl Edge<Player> + use<'event>>, SeekError> {
        context.note("handler Active");
        match event {
            Event::Seek(text) => {
                let position: u32 = text.parse()?;
                Ok(Next::Goto(ActiveEdges::ToPlaying { position }))
            }
            Event::Resume => Ok(Next::Goto(ActiveEdges::ToPlaying { position: 0 })),
            Event::Play(track) => {
                let mut attempts = 0;
                let next = loop {
                    attempts += 1;
                    if attempts == 2 {
                        break Next::Goto(ActiveEdges::ReenterActive);
                    }
                };
                *up.track = *track;
                Ok(next)
            }
            _ => Ok(Next::Unhandled),
        }
    }

    // #[on(Paused)]
    // fn paused(event: &Event) -> Next<Self> { match event { Event::Tick => stay!(), _ => bubble!() } }
    //
    // No `goto!`: the macro writes the concrete `NoEdge` type, so nothing is
    // left to infer.
    pub fn paused(context: &mut Context, event: &Event) -> Next<crate::core::NoEdge> {
        context.note("handler Paused");
        match event {
            Event::Tick => Next::Stay,
            _ => Next::Bubble,
        }
    }
}
