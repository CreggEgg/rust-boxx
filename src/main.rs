use std::{
    borrow::BorrowMut,
    collections::HashMap,
    fmt::write,
    fs,
    hash::Hash,
    ops::Deref,
    slice::Iter,
    str::FromStr,
    sync::{mpsc, Arc, RwLock},
    thread,
    time::{Duration, SystemTime},
};

use rdev::{Button, EventType, Key};
use serde::{de, Deserialize, Serialize};
use vigem_client::XButtons;

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize)]
enum Mode {
    FPS,
    SideScrolling,
}

#[derive(Debug)]
enum ModeError {
    InvalidMode,
}

impl FromStr for Mode {
    type Err = ModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fps" => Ok(Mode::FPS),
            "side-scrolling" => Ok(Mode::SideScrolling),
            _ => Err(ModeError::InvalidMode),
        }
    }
}

#[derive(Serialize, Debug)]
struct XButton(u16);

impl Deref for XButton {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct XButtonVisitor;

impl<'de> de::Visitor<'de> for XButtonVisitor {
    type Value = XButton;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a XButton name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(XButton(match v {
            "UP" => XButtons!(UP).raw,
            "DOWN" => XButtons!(DOWN).raw,
            "LEFT" => XButtons!(LEFT).raw,
            "RIGHT" => XButtons!(RIGHT).raw,
            "START" => XButtons!(START).raw,
            "BACK" => XButtons!(BACK).raw,
            "LTHUMB" => XButtons!(LTHUMB).raw,
            "RTHUMB" => XButtons!(RTHUMB).raw,
            "LB" => XButtons!(LB).raw,
            "RB" => XButtons!(RB).raw,
            "GUIDE" => XButtons!(GUIDE).raw,
            "A" => XButtons!(A).raw,
            "B" => XButtons!(B).raw,
            "X" => XButtons!(X).raw,
            "Y" => XButtons!(Y).raw,
            _ => XButtons!().raw,
        }))
    }
}

impl<'de> Deserialize<'de> for XButton {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(XButtonVisitor)
    }
}

#[derive(Deserialize, Serialize, Debug)]
enum Output {
    Button(XButton),
    Trigger(Trigger, f64),
    Modifier(Axis, f64),
    Axis(Axis, f64),
}
#[derive(Hash, PartialEq, Eq, Deserialize, Serialize, Debug)]
enum Input {
    Key(Key),
    MouseButton(Button), // MousePress(rdev::Button),
                         // MouseRelease(rdev::Button),
                         // MouseMove { delta_x: f64, delta_y: f64 },
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
enum Axis {
    LX,
    LY,
    RX,
    RY,
}

impl Axis {
    pub fn iterator() -> Iter<'static, Axis> {
        [Axis::LX, Axis::LY, Axis::RX, Axis::RY].iter()
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
enum Trigger {
    Left,
    Right,
}

impl Trigger {
    pub fn iterator() -> Iter<'static, Trigger> {
        [Trigger::Right, Trigger::Left].iter()
    }
}

#[derive(Deserialize, Serialize)]
struct Config {
    sensitivity: f64,
    mode: Mode,
    log_keynames: bool,
    binds: HashMap<Mode, (HashMap<Key, Output>, HashMap<Button, Output>)>,
}
struct Pause(bool);

fn main() {
    let mut config: Config =
        serde_json::from_str(&fs::read_to_string("./config.json").unwrap()).unwrap();
    // let sensitivity = config["sensitivity"].as_float().unwrap() * 1000.0;
    // let mode = Mode::from_str(config["mode"].as_str().unwrap()).unwrap();
    // let log_keynames = config["log_keynames"].as_bool().unwrap();
    //

    let (tx, rx) = mpsc::channel();
    let (pause_sender, pause_reciever) = mpsc::channel::<bool>();
    let paused = Arc::new(RwLock::new(Pause(true)));
    {
        let paused = Arc::clone(&paused);
        thread::spawn(move || {
            while let Ok(val) = pause_reciever.recv() {
                println!("Pause");
                paused.write().unwrap().0 = val;
            }
        });
    }
    {
        let tx = tx.clone();
        thread::spawn(move || {
            rdev::grab(move |e| {
                if e.event_type == EventType::KeyPress(Key::Delete) {
                    panic!("Del was pressed... Exiting");
                } else if e.event_type == EventType::KeyPress(Key::Minus) {
                    let current: bool = paused.read().unwrap().0;
                    pause_sender.send(!current).unwrap();
                }

                if !paused.read().unwrap().0 {
                    tx.send(e.event_type).unwrap();
                    None
                } else {
                    Some(e)
                }
            })
            .unwrap();
        });
    }

    let client = vigem_client::Client::connect().unwrap();

    let id = vigem_client::TargetId::XBOX360_WIRED;
    let mut target = vigem_client::Xbox360Wired::new(client, id);

    target.plugin().unwrap();
    target.wait_ready().unwrap();

    let mut gamepad = vigem_client::XGamepad {
        // buttons: vigem_client::XButtons {
        //     raw: vigem_client::XButtons!().into(),
        // },
        buttons: vigem_client::XButtons { raw: 0 },
        ..Default::default()
    };

    // let keymap = {
    //     let mut map: HashMap<Key, Output> = HashMap::new();
    //     match config.mode {
    //         Mode::FPS => {
    //             map.insert(Key::KeyH, Output::Button(XButtons::B));
    //         }
    //         Mode::SideScrolling => {
    //             map.insert(Key::KeyJ, Output::Button(XButtons::B));
    //             map.insert(Key::KeyN, Output::Button(XButtons::A));
    //
    //             map.insert(Key::KeyW, Output::Axis(Axis::LY, 1.0));
    //             map.insert(Key::KeyS, Output::Axis(Axis::LY, -1.0));
    //
    //             map.insert(Key::KeyD, Output::Axis(Axis::LX, 1.0));
    //             map.insert(Key::KeyA, Output::Axis(Axis::LX, -1.0));
    //
    //             map.insert(Key::KeyK, Output::Trigger(Trigger::Right, 1.0));
    //
    //             map.insert(Key::Return, Output::Button(XButtons::START));
    //
    //             map.insert(Key::KeyL, Output::Button(XButtons::X));
    //             map.insert(Key::Space, Output::Modifier(Axis::LY, 0.5));
    //             // map.insert(Key::Space, Output::Trigger(Trigger::Left, 1.0));
    //         }
    //     };
    //     map
    // };

    let handle = thread::spawn(move || {
        let mut keystates: HashMap<Input, KeyState> = HashMap::new();
        let mut last_coords: (f64, f64) = (0.0, 0.0);
        let keymap = process_binds(
            config
                .binds
                .get(&config.mode)
                .expect("No config defined for desired mode"),
        );
        loop {
            if let Ok(event) = rx.try_recv() {
                if let EventType::KeyPress(key) = event {
                    if config.log_keynames {
                        println!("{:?}", key);
                    }
                    keystates.insert(Input::Key(key), KeyState::Pressed(SystemTime::now()));
                }
                if let EventType::KeyRelease(key) = event {
                    keystates.insert(Input::Key(key), KeyState::Released);
                }
                if let EventType::ButtonPress(button) = event {
                    if config.log_keynames {
                        println!("{:?}", button);
                    }
                    keystates.insert(
                        Input::MouseButton(button),
                        KeyState::Pressed(SystemTime::now()),
                    );
                }
                if let EventType::ButtonRelease(button) = event {
                    keystates.insert(Input::MouseButton(button), KeyState::Released);
                }

                let mut buttonflags = 0b0;
                let buttons = keymap.iter().filter_map(|(key, button)| {
                    if let Some(KeyState::Pressed(time)) = keystates.get(key) {
                        Some((SystemTime::now(), button))
                    } else {
                        None
                    }
                });
                let mut axes: HashMap<Axis, (SystemTime, f64)> = HashMap::new();
                for axis in Axis::iterator() {
                    axes.insert(axis.clone(), (SystemTime::UNIX_EPOCH, 0.0));
                }

                let mut modifiers = Vec::new();

                gamepad.left_trigger = 0;
                gamepad.right_trigger = 0;

                buttons.for_each(|output| match output {
                    (_, Output::Button(button)) => buttonflags |= **button,
                    (_, Output::Modifier(axis, value)) => modifiers.push((axis, value)),
                    (time, Output::Axis(axis, value)) => {
                        let current = axes.get(axis);
                        if let Some((current_time, _)) = current {
                            if time
                                .duration_since(*current_time)
                                .map(|x| x.as_secs() as f64)
                                .unwrap_or(-1.0)
                                > 0.0
                            {
                                axes.insert(axis.clone(), (time, *value));
                            }
                        } else {
                            axes.insert(axis.clone(), (time, *value));
                        }
                    }

                    (_, Output::Trigger(trigger, value)) => {
                        match trigger {
                            Trigger::Left => {
                                gamepad.left_trigger = (value * (u8::MAX as f64)) as u8
                            }
                            Trigger::Right => {
                                gamepad.right_trigger = (value * (u8::MAX as f64)) as u8
                            }
                        };
                    }
                });

                for (axis, value) in modifiers {
                    if let Some(axis) = axes.get_mut(axis) {
                        axis.1 *= value;
                    }
                }

                gamepad.buttons.raw = buttonflags;
                // dbg!(gamepad.buttons.raw);

                // let buttons = keystates.iter().filter_map(|(key, state)| match state {
                //     KeyState::Pressed(_) => Some({
                //         let
                //     }),
                //     KeyState::Released => None,
                // });

                // let a = keystates.get(&Key::KeyA).unwrap_or(&KeyState::Released);
                // let d = keystates.get(&Key::KeyD).unwrap_or(&KeyState::Released);
                // let w = keystates.get(&Key::KeyW).unwrap_or(&KeyState::Released);
                // let s = keystates.get(&Key::KeyS).unwrap_or(&KeyState::Released);
                //
                // let lx = handle_socd(d, a);
                // let ly = handle_socd(w, s);
                //
                let lx = axes.get(&Axis::LX).unwrap().1;
                let ly = axes.get(&Axis::LY).unwrap().1;
                let rx = axes.get(&Axis::RX).unwrap().1;
                let ry = axes.get(&Axis::RY).unwrap().1;

                gamepad.thumb_lx = (lx * (i16::MAX as f64)) as i16;
                gamepad.thumb_ly = (ly * (i16::MAX as f64)) as i16;

                if config.mode != Mode::FPS {
                    gamepad.thumb_rx = (rx * (i16::MAX as f64)) as i16;
                    gamepad.thumb_ry = (ry * (i16::MAX as f64)) as i16;
                } else {
                    if let EventType::MouseMove { x, y } = event {
                        // println!("{x}, {y}");
                        let coords = (x, y);

                        let delta = (x - last_coords.0, y - last_coords.1);

                        let new_rx = (delta.0 * config.sensitivity);
                        let new_ry = (delta.1 * -config.sensitivity);
                        let old_rx = gamepad.thumb_rx as f64;
                        let old_ry = gamepad.thumb_ry as f64;
                        let intermediate_rx = old_rx + (((new_rx - old_rx) as f64) * 1.0);
                        let intermediate_ry = old_ry + (((new_ry - old_ry) as f64) * 1.0);
                        gamepad.thumb_rx = intermediate_rx as i16;
                        gamepad.thumb_ry = intermediate_ry as i16;

                        last_coords = coords;
                    }
                }

                target.update(&gamepad).unwrap();
            } else {
                gamepad.thumb_rx = 0;
                gamepad.thumb_ry = 0;
                target.update(&gamepad).unwrap();
                thread::sleep(Duration::from_millis(20));
            }
            // thread::sleep(Duration::from_millis(20));
        }
    });
    handle.join().unwrap();
}

fn process_binds(
    initial: &(HashMap<Key, Output>, HashMap<Button, Output>),
) -> HashMap<Input, &Output> {
    let keys = initial
        .0
        .iter()
        .map(|(key, output)| (Input::Key(*key), output));
    let buttons = initial
        .1
        .iter()
        .map(|(button, output)| (Input::MouseButton(*button), output));
    let tmp = keys.chain(buttons);
    HashMap::from_iter(tmp)
}

fn handle_socd(pos: &KeyState, neg: &KeyState) -> i32 {
    match (pos, neg) {
        (KeyState::Pressed(pos_time), KeyState::Pressed(neg_time)) => {
            if pos_time
                .duration_since(*neg_time)
                .map(|_| true)
                .unwrap_or(false)
            {
                1
            } else {
                -1
            }
        }
        (KeyState::Pressed(_), KeyState::Released) => 1,
        (KeyState::Released, KeyState::Pressed(_)) => -1,
        (KeyState::Released, KeyState::Released) => 0,
    }
}

#[derive(PartialEq)]
enum KeyState {
    Pressed(SystemTime),
    Released,
}
