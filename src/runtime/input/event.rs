use crossterm::event::Event;

#[cfg(unix)]
use crate::runtime::kitty_dnd::KittyDndEvent;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum RuntimeInputEvent {
    Terminal(Event),
    #[cfg(unix)]
    KittyDnd(KittyDndEvent),
}
