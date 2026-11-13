//! The `Panel` machine of the region tests: a compound state that holds a
//! parallel state with three regions, one of them with a compound state.
#![allow(dead_code, missing_docs, unused_imports)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Power,
    Direct,
    Toggle,
    Menu,
    Back,
    Smash,
    Noise,
    Park,
    Reset,
    Next,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, panel::Panel, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>, defer_cap = 4)]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Off,
        Active(Active),
    }

    /// A compound state that holds the parallel state: the regions are two
    /// levels below it.
    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Active {
        pub uptime: u32,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Standby,
        On(On),
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct On {
        pub volume: u8,
        #[region]
        pub light: Light,
        #[region]
        pub sound: Sound,
        #[region]
        pub screen: Screen,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Dim,
        Bright,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Sound {
        #[initial]
        Mute,
        /// An event that bubbles out of `Beep` stops there.
        #[unhandled(error)]
        Beep { count: u8 },
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Screen {
        #[initial]
        Clock,
        Menu(Menu),
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Menu {
        pub opened: u8,
        #[substates]
        pub page: Page,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Page {
        #[initial]
        Top,
        Inner,
        #[terminal]
        Exit,
    }

    #[handlers]
    impl Panel {
        #[entry(On)]
        fn entry_on(up: OnFields<'_>, cx: &mut Cx<Self>) {
            note(cx, &format!("entry On (volume {})", up.volume));
        }

        #[exit(On)]
        fn exit_on(_up: OnFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit On");
        }

        #[entry(Dim)]
        fn entry_dim(cx: &mut Cx<Self>) {
            note(cx, "entry Dim");
        }

        #[exit(Dim)]
        fn exit_dim(cx: &mut Cx<Self>) {
            note(cx, "exit Dim");
        }

        #[entry(Bright)]
        fn entry_bright(cx: &mut Cx<Self>) {
            note(cx, "entry Bright");
        }

        #[exit(Bright)]
        fn exit_bright(cx: &mut Cx<Self>) {
            note(cx, "exit Bright");
        }

        #[entry(Mute)]
        fn entry_mute(cx: &mut Cx<Self>) {
            note(cx, "entry Mute");
        }

        #[exit(Mute)]
        fn exit_mute(cx: &mut Cx<Self>) {
            note(cx, "exit Mute");
        }

        #[entry(Beep)]
        fn entry_beep(beep: BeepMut<'_>, cx: &mut Cx<Self>) {
            note(cx, &format!("entry Beep ({})", beep.count));
        }

        #[exit(Beep)]
        fn exit_beep(beep: BeepMut<'_>, cx: &mut Cx<Self>) {
            note(cx, &format!("exit Beep ({})", beep.count));
        }

        #[entry(Clock)]
        fn entry_clock(cx: &mut Cx<Self>) {
            note(cx, "entry Clock");
        }

        #[exit(Clock)]
        fn exit_clock(cx: &mut Cx<Self>) {
            note(cx, "exit Clock");
        }

        #[entry(Menu)]
        fn entry_menu(up: MenuFields<'_>, cx: &mut Cx<Self>) {
            note(cx, &format!("entry Menu ({})", up.opened));
        }

        #[exit(Menu)]
        fn exit_menu(_up: MenuFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Menu");
        }

        #[on(Off)]
        fn on_off(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Off handles");
            match event {
                Event::Power => goto!(Active { uptime: 0 }),
                // An explicit path: `light` starts in `Bright`, the other
                // regions in their initials.
                Event::Direct => goto!(Active { uptime: 0 } / On { volume: 5 } / Bright),
                _ => bubble!(),
            }
        }

        #[on(Standby)]
        fn on_standby(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Standby handles");
            match event {
                Event::Power => goto!(On { volume: 3 }),
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(_up: ActiveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Active handles");
            match event {
                Event::Reset => goto!(Off),
                _ => bubble!(),
            }
        }

        /// Reached only when no region consumed or deferred the event.
        #[on(On)]
        fn on_on(
            up: OnFields<'_>,
            active: ActiveFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            note(cx, "On handles");
            match event {
                Event::Power => goto!(Standby),
                Event::Noise => {
                    *up.volume += 1;
                    *active.uptime += 1;
                    stay!()
                }
                _ => bubble!(),
            }
        }

        // Region `light`: the first region, so it never sees what the others
        // did to the event.

        #[on(Dim)]
        fn on_dim(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Dim handles");
            match event {
                Event::Toggle => goto!(Bright),
                // Out of the parallel state: the other regions are skipped.
                Event::Smash => goto!(Off),
                Event::Park => stay!(),
                _ => bubble!(),
            }
        }

        #[on(Bright)]
        fn on_bright(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Bright handles");
            match event {
                Event::Toggle => goto!(Dim),
                _ => bubble!(),
            }
        }

        // Region `sound`: it asks `cx.in_state` about `light`, evaluated
        // before it.

        #[on(Mute)]
        fn on_mute(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Mute handles");
            match event {
                Event::Toggle if cx.in_state(Self::BRIGHT_ID) => goto!(Beep { count: 0 }),
                Event::Park => defer!(),
                _ => bubble!(),
            }
        }

        #[on(Beep)]
        fn on_beep(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Beep handles");
            match event {
                // The constructor form: the value of `Beep` leaves by value
                // and a new one is built from it.
                Event::Toggle => goto!(Beep, |old: Exiting![Beep]| Sound::Beep {
                    count: old.count + 1
                }),
                Event::Next => reenter!(),
                Event::Reset => goto!(Mute),
                _ => bubble!(),
            }
        }

        // Region `screen`: a compound state inside a region.

        #[on(Clock)]
        fn on_clock(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Clock handles");
            match event {
                Event::Menu => goto!(Menu { opened: 1 }),
                _ => bubble!(),
            }
        }

        #[on(Top)]
        fn on_top(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Top handles");
            match event {
                Event::Next => goto!(Inner),
                _ => bubble!(),
            }
        }

        #[on(Inner)]
        fn on_inner(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Inner handles");
            match event {
                Event::Next => goto!(Exit),
                _ => bubble!(),
            }
        }

        #[on(Menu)]
        fn on_menu(up: MenuFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Menu handles");
            match event {
                Event::Back => {
                    *up.opened += 1;
                    goto!(Clock)
                }
                _ => bubble!(),
            }
        }

        #[on_done(Menu)]
        fn menu_done(up: MenuFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, &format!("Menu done ({})", up.opened));
            goto!(Clock)
        }
    }
}

pub use panel::{Active, Light, Menu, Mode, On, Page, Panel, Screen, Sound};

pub fn lines(machine: &mut Machine<Panel>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

pub fn off() -> Machine<Panel> {
    let mut machine = Machine::<Panel>::new(Vec::new());
    machine.start().unwrap();
    lines(&mut machine);
    machine
}

/// `Active/On` with every region in its initial, the log cleared.
pub fn on() -> Machine<Panel> {
    let mut machine = off();
    machine.send(Event::Power).unwrap();
    machine.send(Event::Power).unwrap();
    lines(&mut machine);
    machine
}

pub fn path(machine: &Machine<Panel>) -> String {
    machine.path().to_string()
}

pub const INITIALS: &str = "Active/On/light:Dim,sound:Mute,screen:Clock";
