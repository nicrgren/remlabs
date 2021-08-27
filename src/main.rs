use std::{convert::TryFrom, fmt};

mod event_source;
mod raw_event;

use {event_source::EventSource, raw_event::RawEvent};

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
    let ev_src = EventSource::<Event>::open(target).await?;
    let mut tool_events = ToolEventSource::from(ev_src);

    println!("Starting loop");

    loop {
        match tool_events.next().await {
            Ok(tool_ev) => println!("{:?}", tool_ev),
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ToolEvent {
    Update(Tool),
    Removed(ToolKind),
}

/// This should be generalised by making struct Pen
/// be a struct Tool that has a kind field.
/// For now, to get stuff going, just run with it as a Pen.
struct ToolEventSource {
    event_source: EventSource<Event>,
    state: Option<ToolBuilder>,
}

impl ToolEventSource {
    pub fn from(event_source: EventSource<Event>) -> Self {
        Self {
            event_source,
            state: None,
        }
    }

    pub async fn next(&mut self) -> anyhow::Result<ToolEvent> {
        // Currently we need to listen to Tool events.
        // Tool::Pen(true) means the Pen is close to the Pad, start to build.
        // Tool::Pen(false) means the Pen was lifted and we need to reset.
        //
        // These events dictate what we should do.
        // @TODO: Handle these events properly.

        // Wait for an Appeared event.
        let state = if let Some(ref mut state) = self.state {
            state
        } else {
            // Wait for an add event.
            loop {
                match self.event_source.next().await? {
                    Event::ToolAdded(ToolKind::Pen) => {
                        let new_state = ToolBuilder::new(ToolKind::Pen);
                        break self.state.insert(new_state);
                    }

                    Event::ToolAdded(kind) => {
                        eprintln!("Ignoring add event for kind `{:?}`", kind);
                    }

                    ev => {
                        eprintln!("Ignoring Event: {:?}", ev);
                    }
                }
            }
        };
        loop {
            match self.event_source.next().await? {
                Event::Movement(mv) => match state {
                    ToolBuilder::Building(unfinished) => unfinished.apply_movement(mv),
                    ToolBuilder::Built(tool) => tool.apply_movement(mv),
                },

                Event::Sync => match state {
                    ToolBuilder::Building(unfinished) => {
                        let tool = unfinished.finish().expect("Could not finish tool");
                        *state = ToolBuilder::Built(tool);
                        return Ok(ToolEvent::Update(tool));
                    }

                    ToolBuilder::Built(tool) => {
                        return Ok(ToolEvent::Update(*tool));
                    }
                },
                Event::ToolRemoved(ToolKind::Pen) => {
                    self.state = None;
                    return Ok(ToolEvent::Removed(ToolKind::Pen));
                }

                Event::ToolRemoved(kind) => {
                    // Should really wait for a sync....
                    eprintln!("Ignoring non Pen ToolRemoved({:?})", kind);
                }

                Event::ToolAdded(kind) => {
                    eprintln!("Ignoring ToolAdded({:?})", kind);
                }
            }
        }
    }
}

enum ToolBuilder {
    Built(Tool),
    Building(UnfinishedTool),
}

impl ToolBuilder {
    fn new(kind: ToolKind) -> Self {
        Self::Building(UnfinishedTool::kind(kind))
    }
}

struct UnfinishedTool {
    kind: ToolKind,
    x: Option<u32>,
    y: Option<u32>,
    tilt_x: Option<u32>,
    tilt_y: Option<u32>,
    pressure: Option<u32>,
    distance: Option<u32>,
}

impl UnfinishedTool {
    fn kind(kind: ToolKind) -> Self {
        Self {
            kind,
            x: None,
            y: None,
            tilt_x: None,
            tilt_y: None,
            pressure: None,
            distance: None,
        }
    }

    fn apply_movement(&mut self, mv: Movement) {
        match mv {
            Movement::X(n) => self.x.replace(n),
            Movement::Y(n) => self.y.replace(n),
            Movement::TiltX(n) => self.tilt_x.replace(n),
            Movement::TiltY(n) => self.tilt_y.replace(n),
            Movement::Pressure(n) => self.pressure.replace(n),
            Movement::Distance(n) => self.distance.replace(n),
        };
    }

    // @TODO: Change Return type to Result with an error saying what field is missing.
    fn finish(&mut self) -> Option<Tool> {
        let x = self.x.take().expect("Missing X");
        let y = self.y.take().expect("Missing Y");

        let pressure = self.pressure.take().unwrap_or(0);
        let distance = self.distance.take().unwrap_or(0);

        Some(Tool {
            kind: self.kind,
            point: Point(x, y),
            tilt_x: self.tilt_x.take(),
            tilt_y: self.tilt_y.take(),
            height: if distance < 10 && 700 < pressure {
                Height::Touching(pressure)
            } else {
                Height::Distance(distance)
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tool {
    kind: ToolKind,
    point: Point,
    tilt_x: Option<u32>,
    tilt_y: Option<u32>,
    height: Height,
}

impl Tool {
    fn apply_movement(&mut self, ev: Movement) {
        match ev {
            Movement::X(n) => self.point.0 = n,
            Movement::Y(n) => self.point.1 = n,
            Movement::TiltX(n) => self.tilt_x = Some(n),
            Movement::TiltY(n) => self.tilt_y = Some(n),
            Movement::Pressure(n) => self.height = Height::Touching(n),
            Movement::Distance(n) => self.height = Height::Distance(n),
        }
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Tool at {}. tilt x{:?} y{:?}. {}",
            self.point, self.tilt_x, self.tilt_y, self.height
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Point(u32, u32);

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.0, self.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ToolKind {
    Pen,
    Rubber,
    Touch,
    Stylus,
    Stylus2,
}

impl ToolKind {
    fn from_code(code: u16) -> Option<ToolKind> {
        match code {
            320 => Some(Self::Pen),
            321 => Some(Self::Rubber),
            330 => Some(Self::Touch),
            331 => Some(Self::Stylus),
            332 => Some(Self::Stylus2),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Movement {
    X(u32),
    Y(u32),
    TiltX(u32),
    TiltY(u32),
    Pressure(u32),
    Distance(u32),
}

// More Movement, needed to parse events from /dev/input/event2
// #define ABS_MT_TOUCH_MAJOR  0x30    /* Major axis of touching ellipse */
// #define ABS_MT_TOUCH_MINOR  0x31    /* Minor axis (omit if circular) */
// #define ABS_MT_WIDTH_MAJOR  0x32    /* Major axis of approaching ellipse */
// #define ABS_MT_WIDTH_MINOR  0x33    /* Minor axis (omit if circular) */
// #define ABS_MT_ORIENTATION  0x34    /* Ellipse orientation */
// #define ABS_MT_POSITION_X   0x35    /* Center X touch position */
// #define ABS_MT_POSITION_Y   0x36    /* Center Y touch position */
// #define ABS_MT_TOOL_TYPE    0x37    /* Type of touching device */
// #define ABS_MT_BLOB_ID      0x38    /* Group a set of packets as a blob */
// #define ABS_MT_TRACKING_ID  0x39    /* Unique ID of initiated contact */
// #define ABS_MT_PRESSURE     0x3a    /* Pressure on contact area */
// #define ABS_MT_DISTANCE     0x3b    /* Contact hover distance */
// #define ABS_MT_TOOL_X       0x3c    /* Center X tool position */
// #define ABS_MT_TOOL_Y       0x3d    /* Center Y tool position */
//
//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Events read from event1 (Only registers the pen)
enum Event {
    Sync,
    ToolAdded(ToolKind),
    ToolRemoved(ToolKind),
    Movement(Movement),
}

impl TryFrom<RawEvent> for Event {
    type Error = UnknownEvent;

    fn try_from(ev: RawEvent) -> Result<Self, Self::Error> {
        match (ev.typ, ev.code) {
            (0, _) => Ok(Event::Sync),
            (1, code) => {
                // type 1 is a tool event.
                // the code tells what tool
                // value tells if it appeared or went away
                let tool = ToolKind::from_code(code).ok_or(UnknownEvent::ToolCode(code))?;
                match ev.value {
                    0 => Ok(Event::ToolRemoved(tool)),
                    1 => Ok(Event::ToolAdded(tool)),
                    v => Err(UnknownEvent::ToolValue(v)),
                }
            }

            (3, 0) => Ok(Event::Movement(Movement::X(ev.value))),
            (3, 1) => Ok(Event::Movement(Movement::Y(ev.value))),
            (3, 24) => Ok(Event::Movement(Movement::Pressure(ev.value))),
            (3, 25) => Ok(Event::Movement(Movement::Distance(ev.value))),
            (3, 26) => Ok(Event::Movement(Movement::TiltX(ev.value))),
            (3, 27) => Ok(Event::Movement(Movement::TiltY(ev.value))),

            (3, _) => Err(UnknownEvent::MovementCode(ev.code)),

            _ => Err(UnknownEvent::Type(ev)),
        }
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnknownEvent {
    ToolCode(u16),
    ToolValue(u32),
    MovementCode(u16),

    Type(RawEvent),
}

impl std::error::Error for UnknownEvent {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl fmt::Display for UnknownEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ToolCode(code) => write!(f, "Unknown tool code`{:#04x}`", code),
            Self::ToolValue(value) => write!(
                f,
                "Unexpected value for Tool event. Should be 0 or 1. Was:`{:#04x}`",
                value
            ),
            Self::MovementCode(code) => write!(f, "Unknown movement code `{:#04x}`", code),
            Self::Type(ev) => write!(
                f,
                "Unknown type: `{:#02x}`, code: `{:#04x}`, value: `{:#08x}`, ",
                ev.typ, ev.code, ev.value
            ),
        }
    }
}
