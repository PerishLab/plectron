use super::{call, point, setup, snapshot};
use plectron::ipc::{Command, Reply};
use plectron::platform::device::{Button, Event, Phase, Point, Signal};

#[test]
fn pointer() {
    let (mut session, device) = setup();
    call(
        &mut session,
        Command::Evaluate {
            unit: 1,
            source: "let transitions = ''; const hoverTarget = document.getElementById('increment'); hoverTarget.addEventListener('pointerenter', () => { transitions += 'enter;'; }); hoverTarget.addEventListener('pointerleave', () => { transitions += 'leave;'; });"
                .into(),
        },
    );
    let (x, y) = point(&snapshot(&mut session), "increment");
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    for position in [Some(Point { x, y }), None] {
        device.0.borrow_mut().signals.push(Signal {
            unit,
            event: Event::Pointer {
                position,
                phase: Phase::Move,
            },
        });
        session.tick().unwrap();
    }
    let reply = call(
        &mut session,
        Command::Evaluate {
            unit: 1,
            source: "transitions;".into(),
        },
    );
    assert!(matches!(reply, Reply::Evaluation { display } if display == "enter;leave;"));

    device.0.borrow_mut().signals.extend([
        Signal {
            unit,
            event: Event::Pointer {
                position: Some(Point { x, y }),
                phase: Phase::Down(Button::Primary),
            },
        },
        Signal {
            unit,
            event: Event::Pointer {
                position: None,
                phase: Phase::Up(Button::Primary),
            },
        },
    ]);
    session.tick().unwrap();
    assert!(matches!(
        call(
            &mut session,
            Command::Evaluate {
                unit: 1,
                source: "count;".into()
            }
        ),
        Reply::Evaluation { display } if display == "0"
    ));
}
