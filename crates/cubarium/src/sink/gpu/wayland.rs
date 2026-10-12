//! Linux Wayland monitor modes; querying outputs creates no surface or window.
#![forbid(unsafe_code)]

use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

#[derive(Default)]
struct Output {
    name: u32,
    physical: Option<(u32, u32)>,
    rotated: bool,
}

#[derive(Default)]
struct Events {
    outputs: Vec<(wl_output::WlOutput, Output)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Events {
    fn event(
        state: &mut Self, registry: &wl_registry::WlRegistry, event: wl_registry::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } if interface == "wl_output" => {
                state.outputs.push((registry.bind(name, version.min(4), qh, ()),
                                    Output { name, ..Output::default() }));
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.retain(|(_, output)| output.name != name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Events {
    fn event(
        state: &mut Self, output: &wl_output::WlOutput, event: wl_output::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>,
    ) {
        let Some((_, data)) = state.outputs.iter_mut().find(|(proxy, _)| proxy == output) else {
            return;
        };
        match event {
            wl_output::Event::Mode { flags: WEnum::Value(flags), width, height, .. }
                if flags.contains(wl_output::Mode::Current) && width > 0 && height > 0 =>
            {
                data.physical = Some((width as u32, height as u32));
            }
            wl_output::Event::Geometry { transform: WEnum::Value(transform), .. } => {
                data.rotated = matches!(transform, wl_output::Transform::_90 | wl_output::Transform::_270
                    | wl_output::Transform::Flipped90 | wl_output::Transform::Flipped270);
            }
            _ => {}
        }
    }
}

pub fn physical_outputs() -> Option<Vec<(u32, u32)>> {
    if !std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty()) {
        return None;
    }
    let connection = Connection::connect_to_env().ok()?;
    let mut queue = connection.new_event_queue();
    let mut events = Events::default();
    let _registry = connection.display().get_registry(&queue.handle(), ());
    queue.roundtrip(&mut events).ok()?;
    queue.roundtrip(&mut events).ok()?;
    let sizes: Vec<_> = events.outputs.iter().filter_map(|(_, output)| {
        output.physical.map(|(w, h)| if output.rotated { (h, w) } else { (w, h) })
    }).collect();
    (!sizes.is_empty()).then_some(sizes)
}
