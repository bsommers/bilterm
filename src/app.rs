use std::io;
use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::error::Result;
use crate::input::{handle_key_event, AppAction};
use crate::pane::CompositePane;
use crate::pty::{PtyConfig, PtyEvent};
use crate::ui::CompositePaneWidget;

/// Top-level Bilterm TUI application state and async event loop coordinator.
pub struct App {
    pub pane: CompositePane,
    pub should_quit: bool,
}

impl App {
    pub fn new(pane: CompositePane) -> Self {
        Self {
            pane,
            should_quit: false,
        }
    }

    pub fn spawn_default(rows: u16, cols: u16) -> Result<Self> {
        let cfg = PtyConfig {
            rows,
            cols,
            ..Default::default()
        };
        let pane = CompositePane::spawn(1, cfg)?;
        Ok(Self::new(pane))
    }

    /// Runs the main async `tokio::select!` loop multiplexing Crossterm input events,
    /// PTY output chunks, and line-discipline telemetry ticks.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let mut reader = EventStream::new();
        let mut tick = tokio::time::interval(Duration::from_millis(50));

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let _ = self.pane.resize_viewport(area);
                frame.render_widget(CompositePaneWidget::new(&self.pane, true), area);
            })?;

            if self.should_quit || self.pane.has_exited() {
                break;
            }

            tokio::select! {
                maybe_event = reader.next() => {
                    if let Some(Ok(event)) = maybe_event {
                        self.handle_crossterm_event(event)?;
                    }
                }
                pty_ev = async {
                    if let Some(pty) = &mut self.pane.pty {
                        pty.output_rx_mut().recv().await
                    } else {
                        futures::future::pending().await
                    }
                } => {
                    if let Some(ev) = pty_ev {
                        match ev {
                            PtyEvent::Output(bytes) => {
                                self.pane.process_pty_output(&bytes);
                                self.pane.poll_pty_events();
                            }
                            PtyEvent::Exited(_) => {
                                self.should_quit = true;
                            }
                        }
                    }
                }
                _ = tick.tick() => {
                    self.pane.refresh_line_discipline();
                }
                _ = tokio::signal::ctrl_c() => {
                    self.should_quit = true;
                }
            }
        }

        Ok(())
    }

    pub fn handle_crossterm_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if handle_key_event(&mut self.pane, key)? == AppAction::Quit {
                    self.should_quit = true;
                }
            }
            Event::Resize(cols, rows) => {
                let _ = self.pane.resize_viewport(Rect::new(0, 0, cols, rows));
            }
            _ => {}
        }
        Ok(())
    }
}
