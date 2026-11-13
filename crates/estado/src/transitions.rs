//! The transition macros of spec 6.3, as `compile_error!` stubs.
//!
//! `#[machine]` finds every real use inside a `#[handlers]` function and
//! replaces it before it expands. A definition that does expand is therefore
//! a misuse: it reports `E0040` at the call site.

/// Edge to a state of the machine (spec 6.3). Only valid inside a
/// `#[handlers]` function of a `#[machine]` module.
///
/// Forms: `goto!(T)`, `goto!(T { .. })`, `goto!(T(x))`,
/// `goto!(A { .. } / B { .. })`, `goto!(T, |old: Exiting![X]| expr)`,
/// `goto!(T, history)`, `goto!(T, external)`.
///
/// ```ignore
/// #[on(Paused)]
/// fn paused(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Resume => goto!(Playing { pos: Duration::ZERO }),
///         _ => bubble!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::goto!(Playing);
/// }
/// ```
#[macro_export]
macro_rules! goto {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `goto!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// Transition to a state computed at run time (spec 6.3). Only valid inside
/// a `#[handlers]` function of a `#[machine]` module.
///
/// Not supported yet: inside a handler it is error `E0090`.
///
/// The argument is an already built value of the machine; the least common
/// ancestor is computed at run time and the graph records a `*` edge.
///
/// ```ignore
/// #[on(Idle)]
/// fn idle(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Restore(saved) => goto_dyn!(saved.clone()),
///         _ => unhandled!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler(target: u8) {
///     estado::goto_dyn!(target);
/// }
/// ```
#[macro_export]
macro_rules! goto_dyn {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `goto_dyn!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// The event was handled and the state does not change (`Next::Stay`).
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Playing)]
/// fn playing(state: &mut Playing, event: &Ev) -> Next<Self> {
///     if let Ev::Tick(delta) = event {
///         state.position += *delta;
///         return stay!();
///     }
///     bubble!()
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::stay!();
/// }
/// ```
#[macro_export]
macro_rules! stay {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `stay!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// Leave the event to the parent state's handler (`Next::Bubble`).
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Playing)]
/// fn playing(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Pause => goto!(Paused),
///         _ => bubble!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::bubble!();
/// }
/// ```
#[macro_export]
macro_rules! bubble {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `bubble!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// The event is not handled here or by any ancestor, so the machine's
/// unhandled-event policy applies (`Next::Unhandled`, spec 5.7).
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Closed)]
/// fn closed(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Open => goto!(Open),
///         _ => unhandled!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::unhandled!();
/// }
/// ```
#[macro_export]
macro_rules! unhandled {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `unhandled!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// Keep the event in the defer queue until the machine reaches a state that
/// does not defer it (`Next::Defer`, spec 5.5).
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Connecting)]
/// fn connecting(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Connected => goto!(Online),
///         Ev::Send(_) => defer!(),
///         _ => bubble!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::defer!();
/// }
/// ```
#[macro_export]
macro_rules! defer {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `defer!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// Exit and re-enter the current state (`Next::Reenter`, spec 5.2).
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Backoff)]
/// fn backoff(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Retry => reenter!(),
///         _ => bubble!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() {
///     estado::reenter!();
/// }
/// ```
#[macro_export]
macro_rules! reenter {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `reenter!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// The type of a helper that declares its edges: `Next<Self, edges![A | B]>`
/// (spec 6.3). Only valid in the signature of a function of a `#[handlers]`
/// impl, next to `#[edges(A | B)]`.
///
/// Not supported yet: inside a handler it is error `E0090`.
///
/// ```ignore
/// #[edges(Paused | Stopped)]
/// fn pause_or_stop(stop: bool) -> Next<Self, edges![Paused | Stopped]> {
///     if stop { goto!(Stopped) } else { goto!(Paused) }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler() -> estado::edges![A | B] {
///     unreachable!()
/// }
/// ```
#[macro_export]
macro_rules! edges {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `edges!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}

/// The owned exit segment received by the constructor form of `goto!`
/// (spec 6.3): `goto!(T, |old: Exiting![X]| expr)`.
/// Only valid inside a `#[handlers]` function of a `#[machine]` module.
///
/// ```ignore
/// #[on(Playing)]
/// fn playing(event: &Ev) -> Next<Self> {
///     match event {
///         Ev::Fail(reason) => goto!(Failed, |old: Exiting![Active]| Failed {
///             reason: reason.clone(),
///             buffer: old.buf,
///         }),
///         _ => bubble!(),
///     }
/// }
/// ```
///
/// Anywhere else it is error `E0040`:
///
/// ```compile_fail
/// fn not_a_handler(_old: estado::Exiting![Active]) {}
/// ```
#[macro_export]
macro_rules! Exiting {
    ($($tokens:tt)*) => {
        ::core::compile_error!(
            "estado::E0040: `Exiting!` is only valid inside a `#[handlers]` function of a `#[machine]` module"
        )
    };
}
