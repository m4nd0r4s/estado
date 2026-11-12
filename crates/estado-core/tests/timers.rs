//! AC-04 and the runtime contract (spec 7.1-7.3) on the hand-written
//! `Fetch` machine: epochs, stale timer and invoke results, `Driver`.

mod support;

use core::time::Duration;

use estado_core::{Driver, DriverSink, Effect, IgnoreReason, InvokeKey, Machine, TimerKey};
use support::fetch::{Command, Event, Fetch, invoke_key, timer_key};

fn machine() -> Machine<Fetch> {
    let mut machine = Machine::<Fetch>::new(Vec::new());
    machine.start().unwrap();
    machine
}

fn effects(
    step: estado_core::Step<estado_core::InlineEffects<Command, 4>>,
) -> Vec<Effect<Command>> {
    step.effects.into_iter().collect()
}

fn handled_lines(machine: &mut Machine<Fetch>) -> usize {
    std::mem::take(machine.ctx_mut())
        .iter()
        .filter(|line| line.contains("handles"))
        .count()
}

#[test]
fn entering_a_timed_state_starts_its_timer_with_a_new_epoch() {
    let mut machine = machine();
    let step = machine.send(Event::Connect).unwrap();
    assert_eq!(
        effects(step),
        [Effect::StartTimer {
            key: timer_key(1),
            after: Duration::from_millis(100)
        }]
    );
}

#[test]
fn reentering_cancels_the_old_instance_before_starting_the_new_one() {
    let mut machine = machine();
    machine.send(Event::Connect).unwrap();
    let step = machine.fire(timer_key(1)).unwrap();
    assert_eq!(
        effects(step),
        [
            Effect::CancelTimer(timer_key(1)),
            Effect::StartTimer {
                key: timer_key(2),
                after: Duration::from_millis(200)
            }
        ]
    );
    assert_eq!(*machine.state(), Fetch::Connecting { attempt: 2 });
}

#[test]
fn ac_04_timer_of_an_old_epoch_is_stale_and_calls_no_handler() {
    let mut machine = machine();
    machine.send(Event::Connect).unwrap();
    machine.fire(timer_key(1)).unwrap();
    machine.ctx_mut().clear();

    let report = machine.fire(timer_key(1)).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(report.microsteps, 0);
    assert_eq!(handled_lines(&mut machine), 0);
    assert_eq!(*machine.state(), Fetch::Connecting { attempt: 2 });
}

#[test]
fn ac_04_timer_of_a_state_that_exited_is_stale() {
    let mut machine = machine();
    machine.send(Event::Connect).unwrap();
    machine.send(Event::Cancel).unwrap();
    machine.ctx_mut().clear();
    let report = machine.fire(timer_key(1)).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(handled_lines(&mut machine), 0);
}

#[test]
fn ac_04_invoke_result_of_an_old_instance_is_stale() {
    let mut machine = machine();
    machine.send(Event::Load(7)).unwrap();
    machine.send(Event::Cancel).unwrap();
    machine.send(Event::Load(8)).unwrap();
    machine.ctx_mut().clear();

    let report = machine
        .complete(invoke_key(1), Event::Loaded(70))
        .unwrap()
        .report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(handled_lines(&mut machine), 0);

    machine.complete(invoke_key(2), Event::Loaded(80)).unwrap();
    assert_eq!(*machine.state(), Fetch::Ready(80));
}

#[test]
fn ac_04_results_before_start_are_stale() {
    let mut machine = Machine::<Fetch>::new(Vec::new());
    let report = machine.fire(timer_key(1)).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert!(machine.try_state().is_none());
    let report = machine
        .complete(invoke_key(1), Event::Loaded(1))
        .unwrap()
        .report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
}

#[test]
fn invokes_are_started_on_entry_and_cancelled_on_exit() {
    let mut machine = machine();
    let started = effects(machine.send(Event::Load(7)).unwrap());
    assert_eq!(
        started,
        [Effect::Invoke {
            key: invoke_key(1),
            task: Command::Fetch(7)
        }]
    );
    let cancelled = effects(machine.send(Event::Cancel).unwrap());
    assert_eq!(cancelled, [Effect::Cancel(invoke_key(1))]);
}

#[test]
fn the_timer_event_is_built_from_the_state_data_at_fire_time() {
    let mut machine = machine();
    machine.send(Event::Connect).unwrap();
    machine.send(Event::Bump).unwrap();
    machine.ctx_mut().clear();
    machine.fire(timer_key(1)).unwrap();
    assert_eq!(machine.ctx()[0], "Connecting handles Timeout(11)");
    assert_eq!(*machine.state(), Fetch::Idle);
}

#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
}

impl Driver<Command> for Recorder {
    fn effect(&mut self, command: Command) {
        self.calls.push(format!("effect {command:?}"));
    }
    fn start_timer(&mut self, key: TimerKey, after: Duration) {
        self.calls
            .push(format!("start timer {} after {after:?}", key.epoch));
    }
    fn cancel_timer(&mut self, key: TimerKey) {
        self.calls.push(format!("cancel timer {}", key.epoch));
    }
    fn invoke(&mut self, key: InvokeKey, task: Command) {
        self.calls.push(format!("invoke {} {task:?}", key.epoch));
    }
    fn cancel(&mut self, key: InvokeKey) {
        self.calls.push(format!("cancel invoke {}", key.epoch));
    }
}

#[test]
fn a_driver_receives_every_effect_through_driver_sink() {
    let mut machine = machine();
    let mut recorder = Recorder::default();
    machine
        .send_into(Event::Connect, &mut DriverSink(&mut recorder))
        .unwrap();
    machine
        .fire_into(timer_key(1), &mut DriverSink(&mut recorder))
        .unwrap();
    machine
        .send_into(Event::Cancel, &mut DriverSink(&mut recorder))
        .unwrap();
    machine
        .send_into(Event::Load(3), &mut DriverSink(&mut recorder))
        .unwrap();
    assert_eq!(
        recorder.calls,
        [
            "start timer 1 after 100ms",
            "cancel timer 1",
            "start timer 2 after 200ms",
            "cancel timer 2",
            "invoke 1 Fetch(3)"
        ]
    );
}
