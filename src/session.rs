use crate::client::{self, ClientEvent, ClientSession, ConnectOptions, SessionHandle};
use crate::control::{self, Action};
use crate::cursor::Cursor;
use crate::input::{self, Encoder};
use crate::profile::Profile;
use crate::screenshot::{self, CoordinateMap, Screenshot};
use crate::wire::Result;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Disconnected,
    Connecting,
    Connected,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Disconnected => "disconnected",
            Phase::Connecting => "connecting",
            Phase::Connected => "connected",
        }
    }
}

pub struct Session {
    client: Option<ClientSession>,
    events: Option<Receiver<ClientEvent>>,
    phase: Phase,
    status: String,
    partner_id: String,
    view_only: bool,
    width: u32,
    height: u32,
    origin: (i32, i32),
    frames: u64,
    cursor: Option<(i32, i32)>,
    encoder: Option<Encoder>,
    coordinates: Option<CoordinateMap>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            client: None,
            events: None,
            phase: Phase::Disconnected,
            status: "Not connected.".into(),
            partner_id: String::new(),
            view_only: false,
            width: 0,
            height: 0,
            origin: (0, 0),
            frames: 0,
            cursor: None,
            encoder: None,
            coordinates: None,
        }
    }
}

impl Session {
    pub fn connect(&mut self, partner_id: &str, password: &str) -> Result<String> {
        if self.client.is_some() {
            return Err(
                "A session already exists. Disconnect before connecting to another computer."
                    .into(),
            );
        }
        // Fail before any network traffic if the screen cannot be decoded.
        crate::screen::screen_dictionary().map_err(|error| error.to_string())?;
        self.reset();
        self.phase = Phase::Connecting;
        self.partner_id = partner_id.to_string();
        self.status = "Starting the connection.".into();
        let (tx, rx) = mpsc::channel();
        let client = match client::connect_with(
            &Profile::load_or_create()?,
            partner_id,
            password,
            ConnectOptions::default(),
            Box::new(move |event| {
                let _ = tx.send(event);
            }),
        ) {
            Ok(client) => client,
            Err(error) => {
                self.reset();
                return Err(error);
            }
        };
        self.events = Some(rx);
        self.client = Some(client);
        let deadline = Instant::now() + Duration::from_secs(65);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.disconnect();
                return Err("Timed out while connecting to the remote computer.".into());
            }
            let event = self
                .events
                .as_ref()
                .and_then(|events| events.recv_timeout(remaining).ok());
            match event {
                Some(ClientEvent::Connected { .. }) => {
                    self.drain_event(ClientEvent::Connected {
                        width: 0,
                        height: 0,
                    });
                    self.drain();
                    self.ensure_connected()?;
                    return Ok(format!(
                        "Connected to {}. Remote screen: {}×{}. Take a screenshot before sending mouse input.",
                        self.partner_id, self.width, self.height
                    ));
                }
                Some(ClientEvent::Closed(reason)) => {
                    self.drain_event(ClientEvent::Closed(reason.clone()));
                    return Err(reason.into());
                }
                Some(event) => self.drain_event(event),
                None => {
                    self.disconnect();
                    return Err("The connection ended before the remote screen was ready.".into());
                }
            }
        }
    }

    pub fn disconnect(&mut self) -> String {
        let existed = self.client.is_some();
        if let Some(client) = self.client.as_ref() {
            if let Some(encoder) = self.encoder.as_mut() {
                for payload in encoder.release_all() {
                    client.send_input(payload);
                }
            }
            client.close();
        }
        self.reset();
        if existed {
            "Disconnected.".into()
        } else {
            "There is no active session.".into()
        }
    }

    pub fn status(&mut self) -> String {
        self.drain();
        let mut lines = vec![
            format!("Phase: {}", self.phase.name()),
            format!("Status: {}", self.status),
            crate::screen::dictionary::status_line(),
        ];
        if !self.partner_id.is_empty() {
            lines.push(format!("Partner ID: {}", self.partner_id));
        }
        if self.phase == Phase::Connected {
            lines.push(format!("Remote screen: {}×{}", self.width, self.height));
            lines.push(format!(
                "Remote allows control: {}",
                if self.view_only { "no" } else { "yes" }
            ));
            lines.push(format!("Frames received: {}", self.frames));
            if let Some(client) = &self.client {
                lines.push(format!(
                    "Remote UltraViewer version: {}",
                    client.partner_version()
                ));
            }
            lines.push(format!(
                "Screenshot coordinates ready: {}",
                if self.coordinates.is_some() {
                    "yes"
                } else {
                    "no"
                }
            ));
        }
        lines.join("\n")
    }

    pub fn screenshot(&mut self, max_width: u32) -> Result<Screenshot> {
        self.drain();
        self.ensure_connected()?;
        if self.frames == 0 {
            self.wait_for_frame(Duration::from_secs(5))?;
        }
        let client = self
            .client
            .as_ref()
            .ok_or("The remote session is not connected.")?;
        let guard = client.screen.lock().unwrap();
        let renderer = guard
            .as_ref()
            .ok_or("No remote screen is available yet. Try again.")?;
        let image = screenshot::encode(
            renderer.width(),
            renderer.height(),
            renderer.pixels(),
            renderer.init().origin(),
            max_width,
        )?;
        drop(guard);
        self.coordinates = Some(image.coordinates);
        Ok(image)
    }

    pub fn remote_point(&mut self, x: i64, y: i64) -> Result<(i32, i32)> {
        self.drain();
        self.ensure_control()?;
        self.coordinates
            .ok_or("Take a screenshot first; mouse coordinates refer to the latest screenshot.")?
            .remote_point(x, y)
    }

    pub fn cursor_point(&self) -> Option<(i32, i32)> {
        self.cursor
            .map(|(x, y)| (x + self.origin.0, y + self.origin.1))
    }

    pub fn input<F>(&mut self, build: F) -> Result<()>
    where
        F: FnOnce(&mut Encoder) -> Vec<Vec<u8>>,
    {
        self.drain();
        self.ensure_control()?;
        let handle = self.handle()?;
        let encoder = self.encoder.as_mut().ok_or("Remote input is not ready.")?;
        for payload in build(encoder) {
            handle.send_input(payload);
        }
        Ok(())
    }

    pub fn remote_action(&mut self, action: Action) -> Result<()> {
        self.drain();
        self.ensure_control()?;
        let handle = self.handle()?;
        if let Some(encoder) = self.encoder.as_mut() {
            for payload in encoder.release_all() {
                handle.send_input(payload);
            }
        }
        let (payload, format_flag) = control::action(action);
        handle.send_command(payload, format_flag);
        Ok(())
    }

    fn handle(&self) -> Result<SessionHandle> {
        self.client
            .as_ref()
            .map(ClientSession::handle)
            .ok_or_else(|| "The remote session is not connected.".into())
    }

    fn ensure_connected(&self) -> Result<()> {
        if self.phase != Phase::Connected
            || self.client.as_ref().is_none_or(|client| client.is_closed())
        {
            return Err("Not connected to a remote computer. Use connect first.".into());
        }
        Ok(())
    }

    fn ensure_control(&self) -> Result<()> {
        self.ensure_connected()?;
        if self.view_only {
            return Err("The remote computer allows viewing only.".into());
        }
        Ok(())
    }

    fn wait_for_frame(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while self.frames == 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("No screen frame arrived yet. Try again.".into());
            }
            match self
                .events
                .as_ref()
                .and_then(|events| events.recv_timeout(remaining).ok())
            {
                Some(ClientEvent::Closed(reason)) => {
                    self.drain_event(ClientEvent::Closed(reason.clone()));
                    return Err(reason.into());
                }
                Some(event) => self.drain_event(event),
                None => return Err("The session ended before a screen frame arrived.".into()),
            }
        }
        Ok(())
    }

    fn drain(&mut self) {
        loop {
            let event = match self.events.as_ref().map(Receiver::try_recv) {
                Some(Ok(event)) => event,
                Some(Err(TryRecvError::Empty)) | None => break,
                Some(Err(TryRecvError::Disconnected)) => break,
            };
            self.drain_event(event);
        }
    }

    fn drain_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::Status(status) => self.status = status,
            ClientEvent::Connected { width, height } => {
                if let Some(client) = &self.client {
                    if let Some(renderer) = client.screen.lock().unwrap().as_ref() {
                        self.width = renderer.width();
                        self.height = renderer.height();
                        self.origin = renderer.init().origin();
                    } else {
                        self.width = width;
                        self.height = height;
                    }
                    self.encoder = Some(Encoder::new(input::supports_binary_keyboard(
                        &client.partner_version(),
                    )));
                }
                self.phase = Phase::Connected;
                self.status = "Connected.".into();
                self.frames = 0;
                self.coordinates = None;
            }
            ClientEvent::FrameReady => self.frames += 1,
            ClientEvent::Cursor(Cursor::Position { x, y }) => {
                self.cursor = Some((x as i32 - self.origin.0, y as i32 - self.origin.1));
            }
            ClientEvent::ViewOnly(view_only) => self.view_only = view_only,
            ClientEvent::Closed(reason) => {
                self.status = reason;
                self.phase = Phase::Disconnected;
                self.client = None;
                self.events = None;
                self.encoder = None;
                self.coordinates = None;
            }
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.client = None;
        self.events = None;
        self.phase = Phase::Disconnected;
        self.status = "Not connected.".into();
        self.partner_id.clear();
        self.view_only = false;
        self.width = 0;
        self.height = 0;
        self.origin = (0, 0);
        self.frames = 0;
        self.cursor = None;
        self.encoder = None;
        self.coordinates = None;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.disconnect();
    }
}
