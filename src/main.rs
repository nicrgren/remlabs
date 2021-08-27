use std::{convert::TryFrom, fmt};

mod event;
mod event_source;

use {event::Event, event_source::EventSource};

// type Error = Box<dyn StdError + 'static + Send>;
// type Result<T> = std::result::Result<T, Error>;

// type Error = anyhow::Error;
// type Result<T> = anyhow::Result<T>;

fn main() -> anyhow::Result<()> {
    println!("BLALALAL");
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/dev/input/event1"));

    let rt = tokio::runtime::Builder::new_current_thread().build()?;

    rt.block_on(async {
        if let Err(err) = async_main(&target).await {
            eprintln!("Stopping with error: {}", err);
        }
    });

    Ok(())
}

async fn async_main(target: &str) -> anyhow::Result<()> {
    let ev_src = EventSource::<PenEvent>::open(target).await?;
    let mut pen_listener = PenListener::from(ev_src);

    println!("Starting loop");

    loop {
        match pen_listener.next().await {
            Ok(pen) => println!("{}", pen),
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}

struct PenListener {
    event_source: EventSource<PenEvent>,
    state: PenListenerState,
}

impl PenListener {
    pub fn from(event_source: EventSource<PenEvent>) -> Self {
        Self {
            event_source,
            state: PenListenerState::WaitingForFirstPen(PenBuilder::default()),
        }
    }

    pub async fn next(&mut self) -> anyhow::Result<Pen> {
        // Currently we need to listen to Tool events.
        // Tool::Pen(true) means the Pen is close to the Pad, start to build.
        // Tool::Pen(false) means the Pen was lifted and we need to reset.
        //
        // These events dictate what we should do.
        // @TODO: Handle these events properly.

        match self.state {
            PenListenerState::WaitingForFirstPen(ref mut builder) => loop {
                match self.event_source.next().await? {
                    PenEvent::Sync => {
                        // @TODO: Use an intermediate option to remove clone.
                        let pen = builder
                            .build()
                            .ok_or_else(|| anyhow::anyhow!("Received Sync but failed to build"))?;
                        self.state = PenListenerState::PenBuilt(pen);
                        return Ok(pen);
                    }

                    PenEvent::Movement(mv) => builder.apply_event(mv),

                    PenEvent::Tool(Tool::Pen(false)) => {
                        println!("Ignoring Pen lifted");
                    }

                    ev => {
                        println!("Builder ignoring Event: {:?}", ev);
                    }
                }
            },

            PenListenerState::PenBuilt(ref mut pen) => loop {
                match self.event_source.next().await? {
                    PenEvent::Sync => return Ok(*pen),

                    PenEvent::Movement(mv) => {
                        pen.apply_movement(mv);
                    }

                    PenEvent::Tool(Tool::Pen(n)) => {
                        println!("Got Tool Pen({}). Ign", n);
                    }

                    PenEvent::Tool(ev) => {
                        println!("Builder ignoring ToolEvent: {:?}", ev);
                    }
                }
            },
        }
    }
}

enum PenListenerState {
    WaitingForFirstPen(PenBuilder),

    PenBuilt(Pen),
}

#[derive(Default)]
struct PenBuilder {
    x: Option<u32>,
    y: Option<u32>,
    tilt_x: Option<u32>,
    tilt_y: Option<u32>,
    pressure: Option<u32>,
    distance: Option<u32>,
}

impl PenBuilder {
    fn apply_event(&mut self, ev: Movement) {
        match ev {
            Movement::X(n) => self.x = Some(n),
            Movement::Y(n) => self.y = Some(n),
            Movement::TiltX(n) => self.tilt_x = Some(n),
            Movement::TiltY(n) => self.tilt_y = Some(n),
            Movement::Pressure(n) => self.pressure = Some(n),
            Movement::Distance(n) => self.distance = Some(n),
        }
    }

    // @TODO: Fix proper errors here.
    fn build(&mut self) -> Option<Pen> {
        let x = self.x.take().expect("Missing X");
        let y = self.y.take().expect("Missing Y");
        let tx = self.tilt_x.take().expect("Missing Tilt X");
        let ty = self.tilt_y.take().expect("Missing Tilt Y");

        let pressure = self.pressure.take().unwrap_or(0);
        let distance = self.distance.take().unwrap_or(0);

        Some(Pen {
            point: Point(x, y),
            tilt: Point(tx, ty),
            height: if distance < 10 && 700 < pressure {
                Height::Touching(pressure)
            } else {
                Height::Distance(distance)
            },
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Pen {
    point: Point,
    tilt: Point,
    height: Height,
}

impl Pen {
    fn apply_movement(&mut self, ev: Movement) {
        match ev {
            Movement::X(n) => self.point.0 = n,
            Movement::Y(n) => self.point.1 = n,
            Movement::TiltX(n) => self.tilt.0 = n,
            Movement::TiltY(n) => self.tilt.1 = n,
            Movement::Pressure(n) => self.height = Height::Touching(n),
            Movement::Distance(n) => self.height = Height::Distance(n),
        }
    }
}

impl fmt::Display for Pen {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Pen at {}. tilt {}. {}",
            self.point, self.tilt, self.height
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum Height {
    Distance(u32),
    Touching(u32),
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Distance(n) => write!(f, "distance{}", n),
            Self::Touching(n) => write!(f, "touching{}", n),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Point(u32, u32);

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.0, self.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pen(bool), // Read 1 or 0 for all tools
    Rubber,
    Touch,
    Stylus,
    Stylus2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Movement {
    X(u32),
    Y(u32),
    TiltX(u32),
    TiltY(u32),
    Pressure(u32),
    Distance(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Events read from event1 (Only registers the pen)
enum PenEvent {
    Sync,
    Tool(Tool),
    Movement(Movement),
}

impl TryFrom<Event> for PenEvent {
    type Error = UnknownPenEvent;

    fn try_from(ev: Event) -> Result<Self, Self::Error> {
        match (ev.typ, ev.code) {
            (0, _) => Ok(PenEvent::Sync),
            (1, 320) => Ok(PenEvent::Tool(Tool::Pen(ev.value == 1))),
            (1, 321) => Ok(PenEvent::Tool(Tool::Rubber)),
            (1, 330) => Ok(PenEvent::Tool(Tool::Touch)),
            (1, 331) => Ok(PenEvent::Tool(Tool::Stylus)),
            (1, 332) => Ok(PenEvent::Tool(Tool::Stylus2)),
            (1, code) => Err(UnknownPenEvent::ToolCode(code)),

            (3, 0) => Ok(PenEvent::Movement(Movement::X(ev.value))),
            (3, 1) => Ok(PenEvent::Movement(Movement::Y(ev.value))),
            (3, 24) => Ok(PenEvent::Movement(Movement::Pressure(ev.value))),
            (3, 25) => Ok(PenEvent::Movement(Movement::Distance(ev.value))),
            (3, 26) => Ok(PenEvent::Movement(Movement::TiltX(ev.value))),
            (3, 27) => Ok(PenEvent::Movement(Movement::TiltY(ev.value))),
            (3, _) => Err(UnknownPenEvent::MovementCode(ev.code)),

            _ => Err(UnknownPenEvent::Type(ev)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnknownPenEvent {
    ToolCode(u16),
    MovementCode(u16),

    Type(Event),
}

impl std::error::Error for UnknownPenEvent {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl fmt::Display for UnknownPenEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ToolCode(code) => write!(f, "Unknown tool code`{:#04x}`", code),
            Self::MovementCode(code) => write!(f, "Unknown movement code `{:#04x}`", code),
            Self::Type(ev) => write!(
                f,
                "Unknown type: `{:#02x}`, code: `{:#04x}`, value: `{:#08x}`, ",
                ev.typ, ev.code, ev.value
            ),
        }
    }
}
