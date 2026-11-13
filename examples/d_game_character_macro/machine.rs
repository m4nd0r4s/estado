//! The game character as a `#[machine]` module (ADR 0010).
//!
//! Compared with the hand-written scenario: the enums and structs are the
//! same; `#[handlers]` replaces the dispatch of the three regions, the edge
//! enums, the entries and exits, the timers, the views and the graph, all
//! generated. The declaration order of the regions is their evaluation order
//! (ADR 0008): `anim` derives its pose from `movement` and `weapon`, so it is
//! declared last.

use estado_core::{Cx, Next, Sink};

use super::{
    Event, FIRE_COOLDOWN, GameCtx, JUMP_TIME, MAGAZINE_SIZE, MAX_HEALTH, RELOAD_TIME, Sound,
};

/// Effects (commands) the machine emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// The pose changed: start this animation.
    PlayAnimation(character::Anim),
    /// A shot left the barrel.
    SpawnProjectile,
    /// Play a sound.
    PlaySound(Sound),
}

/// Appends a line to the trace kept in the global context.
#[inline]
fn note<S: Sink<Command>>(cx: &mut Cx<'_, character::Character, S>, line: &'static str) {
    cx.ctx_mut().note(line);
}

// Helpers of the handlers, outside the machine module (ADR 0013 item 15): the
// module is one inline unit, so only the machine itself lives in it. `cx.in_state`
// below is out of the macro's sight, hence `tracks_active`.
/// The events after which a pose may have to change: the ones `movement`
/// and `weapon` react to by changing state.
const fn changes_pose(event: &Event) -> bool {
    matches!(
        event,
        Event::Move
            | Event::Stop
            | Event::Jump
            | Event::Land
            | Event::Crouch
            | Event::Stand
            | Event::Fire
            | Event::CooldownElapsed
    )
}

/// The pose the character should have right now: shooting wins over
/// jumping, which wins over running.
fn desired_pose<S: Sink<Command>>(cx: &Cx<'_, character::Character, S>) -> character::Anim {
    if cx.in_state(character::Character::FIRING_ID) {
        character::Anim::Shoot
    } else if cx.in_state(character::Character::JUMPING_ID) {
        character::Anim::Jump
    } else if cx.in_state(character::Character::WALKING_ID) {
        character::Anim::Run
    } else {
        character::Anim::Idle
    }
}

#[estado_macros::machine(
    event = Event,
    effect = Command,
    ctx = GameCtx,
    defer_cap = 2,
    raise_cap = 0,
    defer_overflow = drop_oldest,
    tracks_active
)]
pub mod character {
    use super::*;

    /// The machine: root enum.
    #[root]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Character {
        /// Parallel state with the regions `movement`, `weapon` and `anim`.
        #[initial(with = fresh_alive)]
        Alive(Alive),
        /// Terminal: every event is ignored, except `Respawn`.
        Dead,
    }

    /// `#[initial(with = ..)]` of `Alive`: full health and magazine.
    fn fresh_alive() -> Character {
        Character::Alive(Alive {
            health: MAX_HEALTH,
            ammo: MAGAZINE_SIZE,
            movement: Movement::Standing,
            weapon: Weapon::Ready,
            anim: Anim::Idle,
        })
    }

    /// Parallel state: shared data and three `#[region]`s, in evaluation order.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Alive {
        /// Hit points; zero is never stored, `Damage` leaves `Alive` instead.
        pub health: u16,
        /// Rounds in the magazine, `0..=MAGAZINE_SIZE`.
        pub ammo: u8,
        /// First region.
        #[region]
        pub movement: Movement,
        /// Second region; reads `movement` through `cx.in_state`.
        #[region]
        pub weapon: Weapon,
        /// Third region; reads both others through `cx.in_state`.
        #[region]
        pub anim: Anim,
    }

    /// Region `movement`. The standing state is not called `Idle` because
    /// state names are unique across all levels (spec 3.2, E0012) and `anim`
    /// has one.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Movement {
        /// `#[initial]`.
        #[initial]
        Standing,
        /// Moving on the ground.
        Walking,
        /// In the air.
        #[after(JUMP_TIME => Event::Land)]
        Jumping,
        /// Ducking.
        Crouching,
    }

    /// Region `weapon`.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Weapon {
        /// `#[initial]`; the magazine is never empty here.
        #[initial]
        Ready,
        /// A shot was just fired.
        #[after(FIRE_COOLDOWN => Event::CooldownElapsed)]
        Firing,
        /// Reloading.
        #[after(RELOAD_TIME => Event::ReloadDone)]
        Reloading,
        /// The magazine is empty.
        Empty,
    }

    /// Region `anim`: the pose derived from what the other regions are doing.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Anim {
        /// `#[initial]`.
        #[initial]
        Idle,
        /// Walking.
        Run,
        /// In the air.
        Jump,
        /// Firing; wins over every other pose.
        Shoot,
    }

    #[handlers]
    impl Character {
        // Entries and exits, one per state.

        #[entry(Alive)]
        fn entry_alive(_up: AliveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Alive");
        }

        #[exit(Alive)]
        fn exit_alive(_up: AliveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Alive");
        }

        #[entry(Standing)]
        fn entry_standing(cx: &mut Cx<Self>) {
            note(cx, "entry movement:Standing");
        }

        #[exit(Standing)]
        fn exit_standing(cx: &mut Cx<Self>) {
            note(cx, "exit movement:Standing");
        }

        #[entry(Walking)]
        fn entry_walking(cx: &mut Cx<Self>) {
            note(cx, "entry movement:Walking");
        }

        #[exit(Walking)]
        fn exit_walking(cx: &mut Cx<Self>) {
            note(cx, "exit movement:Walking");
        }

        #[entry(Jumping)]
        fn entry_jumping(cx: &mut Cx<Self>) {
            note(cx, "entry movement:Jumping");
        }

        #[exit(Jumping)]
        fn exit_jumping(cx: &mut Cx<Self>) {
            note(cx, "exit movement:Jumping");
        }

        #[entry(Crouching)]
        fn entry_crouching(cx: &mut Cx<Self>) {
            note(cx, "entry movement:Crouching");
        }

        #[exit(Crouching)]
        fn exit_crouching(cx: &mut Cx<Self>) {
            note(cx, "exit movement:Crouching");
        }

        #[entry(Ready)]
        fn entry_ready(cx: &mut Cx<Self>) {
            note(cx, "entry weapon:Ready");
        }

        #[exit(Ready)]
        fn exit_ready(cx: &mut Cx<Self>) {
            note(cx, "exit weapon:Ready");
        }

        #[entry(Firing)]
        fn entry_firing(cx: &mut Cx<Self>) {
            note(cx, "entry weapon:Firing");
            cx.emit(Command::PlaySound(Sound::Gunshot));
        }

        #[exit(Firing)]
        fn exit_firing(cx: &mut Cx<Self>) {
            note(cx, "exit weapon:Firing");
        }

        #[entry(Reloading)]
        fn entry_reloading(cx: &mut Cx<Self>) {
            note(cx, "entry weapon:Reloading");
            cx.emit(Command::PlaySound(Sound::ReloadStart));
        }

        #[exit(Reloading)]
        fn exit_reloading(cx: &mut Cx<Self>) {
            note(cx, "exit weapon:Reloading");
        }

        #[entry(Empty)]
        fn entry_empty(cx: &mut Cx<Self>) {
            note(cx, "entry weapon:Empty");
        }

        #[exit(Empty)]
        fn exit_empty(cx: &mut Cx<Self>) {
            note(cx, "exit weapon:Empty");
        }

        #[entry(Idle)]
        fn entry_idle(cx: &mut Cx<Self>) {
            note(cx, "entry anim:Idle");
            cx.emit(Command::PlayAnimation(Anim::Idle));
        }

        #[exit(Idle)]
        fn exit_idle(cx: &mut Cx<Self>) {
            note(cx, "exit anim:Idle");
        }

        #[entry(Run)]
        fn entry_run(cx: &mut Cx<Self>) {
            note(cx, "entry anim:Run");
            cx.emit(Command::PlayAnimation(Anim::Run));
        }

        #[exit(Run)]
        fn exit_run(cx: &mut Cx<Self>) {
            note(cx, "exit anim:Run");
        }

        #[entry(Jump)]
        fn entry_jump(cx: &mut Cx<Self>) {
            note(cx, "entry anim:Jump");
            cx.emit(Command::PlayAnimation(Anim::Jump));
        }

        #[exit(Jump)]
        fn exit_jump(cx: &mut Cx<Self>) {
            note(cx, "exit anim:Jump");
        }

        #[entry(Shoot)]
        fn entry_shoot(cx: &mut Cx<Self>) {
            note(cx, "entry anim:Shoot");
            cx.emit(Command::PlayAnimation(Anim::Shoot));
        }

        #[exit(Shoot)]
        fn exit_shoot(cx: &mut Cx<Self>) {
            note(cx, "exit anim:Shoot");
        }

        #[entry(Dead)]
        fn entry_dead(cx: &mut Cx<Self>) {
            note(cx, "entry Dead");
            cx.emit(Command::PlaySound(Sound::Death));
        }

        #[exit(Dead)]
        fn exit_dead(cx: &mut Cx<Self>) {
            note(cx, "exit Dead");
        }

        // Handlers of the root states.

        /// Reached only when no region consumed the event. No region handles
        /// `Damage` or `Heal`, so they always land here; a region that
        /// answered `Damage` with `stay!()` would swallow it.
        ///
        /// Fatal damage is the preemption of the scenario: `goto!(Dead)` has
        /// the root as its LCA, so every region exits, in reverse declaration
        /// order, and every timer of the regions is cancelled.
        #[on(Alive)]
        fn on_alive(up: AliveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Alive");
            match event {
                Event::Damage(amount) => {
                    *up.health = up.health.saturating_sub(*amount);
                    if *up.health == 0 {
                        goto!(Dead)
                    } else {
                        stay!()
                    }
                }
                Event::Heal(amount) => {
                    *up.health = up.health.saturating_add(*amount).min(MAX_HEALTH);
                    stay!()
                }
                _ => bubble!(),
            }
        }

        /// Only `Respawn` brings the character back.
        #[on(Dead)]
        fn on_dead(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Dead");
            match event {
                Event::Respawn => goto!(Alive {
                    health: MAX_HEALTH,
                    ammo: MAGAZINE_SIZE,
                }),
                _ => bubble!(),
            }
        }

        // Region `movement`: the first region to run, so it never sees what
        // the other two did to the event.

        #[on(Standing)]
        fn on_standing(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler movement:Standing");
            match event {
                Event::Move => goto!(Walking),
                Event::Jump => goto!(Jumping),
                Event::Crouch => goto!(Crouching),
                _ => bubble!(),
            }
        }

        /// Moving again while walking changes nothing but is handled, so the
        /// event is consumed.
        #[on(Walking)]
        fn on_walking(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler movement:Walking");
            match event {
                Event::Move => stay!(),
                Event::Stop => goto!(Standing),
                Event::Jump => goto!(Jumping),
                Event::Crouch => goto!(Crouching),
                _ => bubble!(),
            }
        }

        /// No double jump, no air control; only the landing.
        #[on(Jumping)]
        fn on_jumping(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler movement:Jumping");
            match event {
                Event::Land => goto!(Standing),
                _ => bubble!(),
            }
        }

        #[on(Crouching)]
        fn on_crouching(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler movement:Crouching");
            match event {
                Event::Stand => goto!(Standing),
                _ => bubble!(),
            }
        }

        // Region `weapon`: the second region; it asks `cx.in_state` about
        // `movement`, which ran before it.

        /// A reload is refused while the character is airborne by
        /// *deferring* the request: landing changes the configuration, so the
        /// request is redelivered and the reload starts then.
        #[on(Ready)]
        fn on_ready(up: AliveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler weapon:Ready");
            match event {
                Event::Fire => {
                    *up.ammo = up.ammo.saturating_sub(1);
                    cx.emit(Command::SpawnProjectile);
                    goto!(Firing)
                }
                Event::ReloadRequest if *up.ammo == MAGAZINE_SIZE => stay!(),
                Event::ReloadRequest if cx.in_state(Self::JUMPING_ID) => defer!(),
                Event::ReloadRequest => goto!(Reloading),
                _ => bubble!(),
            }
        }

        /// The trigger is locked until the cooldown ends, and a reload waits
        /// for it.
        #[on(Firing)]
        fn on_firing(up: AliveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler weapon:Firing");
            match event {
                Event::CooldownElapsed if *up.ammo == 0 => goto!(Empty),
                Event::CooldownElapsed => goto!(Ready),
                Event::ReloadRequest => defer!(),
                _ => bubble!(),
            }
        }

        /// The timer of this state is the reload; leaving it before it fires
        /// (fatal damage) cancels it.
        #[on(Reloading)]
        fn on_reloading(up: AliveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler weapon:Reloading");
            match event {
                Event::ReloadDone => {
                    *up.ammo = MAGAZINE_SIZE;
                    goto!(Ready)
                }
                Event::ReloadRequest => stay!(),
                _ => bubble!(),
            }
        }

        /// The trigger clicks; a reload follows the airborne rule of `Ready`.
        #[on(Empty)]
        fn on_empty(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler weapon:Empty");
            match event {
                Event::Fire => {
                    cx.emit(Command::PlaySound(Sound::EmptyClick));
                    stay!()
                }
                Event::ReloadRequest if cx.in_state(Self::JUMPING_ID) => defer!(),
                Event::ReloadRequest => goto!(Reloading),
                _ => bubble!(),
            }
        }

        // Region `anim`: the last region, it owns no state of its own. Its
        // pose is a function of what `movement` and `weapon` are doing
        // *after* they handled the same event (ADR 0005, ADR 0008). When the
        // pose is already the right one the handler bubbles instead of
        // answering `stay!()`: `anim` did nothing, so it must not claim the
        // event.

        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler anim:Idle");
            if !changes_pose(event) {
                return bubble!();
            }
            match desired_pose(cx) {
                Anim::Idle => bubble!(),
                Anim::Run => goto!(Run),
                Anim::Jump => goto!(Jump),
                Anim::Shoot => goto!(Shoot),
            }
        }

        #[on(Run)]
        fn on_run(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler anim:Run");
            if !changes_pose(event) {
                return bubble!();
            }
            match desired_pose(cx) {
                Anim::Run => bubble!(),
                Anim::Idle => goto!(Idle),
                Anim::Jump => goto!(Jump),
                Anim::Shoot => goto!(Shoot),
            }
        }

        #[on(Jump)]
        fn on_jump(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler anim:Jump");
            if !changes_pose(event) {
                return bubble!();
            }
            match desired_pose(cx) {
                Anim::Jump => bubble!(),
                Anim::Idle => goto!(Idle),
                Anim::Run => goto!(Run),
                Anim::Shoot => goto!(Shoot),
            }
        }

        #[on(Shoot)]
        fn on_shoot(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler anim:Shoot");
            if !changes_pose(event) {
                return bubble!();
            }
            match desired_pose(cx) {
                Anim::Shoot => bubble!(),
                Anim::Idle => goto!(Idle),
                Anim::Run => goto!(Run),
                Anim::Jump => goto!(Jump),
            }
        }
    }
}
