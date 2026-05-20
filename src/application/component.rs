use std::{
    path::PathBuf,
    sync::{Arc, mpsc::Sender},
};

use enum_dispatch::enum_dispatch;
use ratatui::{Frame, crossterm};

use crate::application::home::{available::AvailableAPDetails, connected::ConnectionData};

/// Read only context, shared across the application
pub struct Context {
    #[allow(unused)]
    pub log_file: PathBuf,
    pub fps: u32,
    pub connection_maxitem: usize,
}

pub struct RichContext {
    #[allow(unused)]
    pub log_file: PathBuf,
    pub fps: u32,
    #[allow(unused)]
    pub message: Arc<Sender<Message>>,
    pub connection_maxitem: usize,
}

pub struct Connected {
    pub list: Vec<Arc<ConnectionData>>,
}

pub struct Available {
    pub list: Vec<Arc<AvailableAPDetails>>,
}

pub struct AuthenticationData {
    pub access_point: Arc<AvailableAPDetails>,
}

pub enum Message {
    Crossterm(crossterm::event::Event),
    GlobalTick,

    // Home pane events
    LoadConnected(Connected),
    LoadAvailable(Available),
    FinishLoading,
    FinishAuthentication,
    Authenticate(AuthenticationData),
}

pub enum Bubble {
    Yes(Message),
    No,
}

#[enum_dispatch]
pub trait Component {
    fn update(&mut self, ctx: &RichContext, ev: Message) -> Bubble;
    fn draw(&mut self, ctx: &RichContext, frame: &mut Frame<'_>);
}
