#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Hidden,
    Visible,
    Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleCommand {
    Show,
    Hide,
    Toggle,
    WindowCloseRequested,
    ExitFromTray,
}

pub trait WindowPort {
    type Error;

    fn show(&mut self) -> Result<(), Self::Error>;
    fn hide(&mut self) -> Result<(), Self::Error>;
    fn quit(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleController {
    state: LifecycleState,
}

impl Default for LifecycleController {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleController {
    pub const fn new() -> Self {
        Self {
            state: LifecycleState::Hidden,
        }
    }

    pub const fn state(&self) -> LifecycleState {
        self.state
    }

    /// Records the native close response. Slint performs the actual hide after
    /// the callback returns `CloseRequestResponse::HideWindow`.
    ///
    /// `Exiting` is terminal: a queued close dispatched during shutdown must not
    /// overwrite the exit state.
    pub fn accept_window_close(&mut self) -> LifecycleState {
        if self.state != LifecycleState::Exiting {
            self.state = LifecycleState::Hidden;
        }
        self.state
    }

    pub fn handle<P: WindowPort>(
        &mut self,
        command: LifecycleCommand,
        window: &mut P,
    ) -> Result<LifecycleState, P::Error> {
        // `Exiting` is terminal. Once the tray Exit command has scheduled
        // termination, queued Show/Hide/Toggle/close callbacks must not revive
        // the window or invoke further platform side effects.
        if self.state == LifecycleState::Exiting {
            return Ok(LifecycleState::Exiting);
        }

        let next_state = match command {
            LifecycleCommand::Show => {
                window.show()?;
                LifecycleState::Visible
            }
            LifecycleCommand::Hide | LifecycleCommand::WindowCloseRequested => {
                window.hide()?;
                LifecycleState::Hidden
            }
            LifecycleCommand::Toggle => match self.state {
                LifecycleState::Hidden => {
                    window.show()?;
                    LifecycleState::Visible
                }
                LifecycleState::Visible => {
                    window.hide()?;
                    LifecycleState::Hidden
                }
                LifecycleState::Exiting => LifecycleState::Exiting,
            },
            LifecycleCommand::ExitFromTray => {
                window.quit()?;
                LifecycleState::Exiting
            }
        };

        self.state = next_state;
        Ok(next_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct FakeWindow {
        actions: Vec<&'static str>,
        fail_next: bool,
    }

    impl WindowPort for FakeWindow {
        type Error = &'static str;

        fn show(&mut self) -> Result<(), Self::Error> {
            if self.fail_next {
                self.fail_next = false;
                return Err("show failed");
            }
            self.actions.push("show");
            Ok(())
        }

        fn hide(&mut self) -> Result<(), Self::Error> {
            self.actions.push("hide");
            Ok(())
        }

        fn quit(&mut self) -> Result<(), Self::Error> {
            if self.fail_next {
                self.fail_next = false;
                return Err("quit failed");
            }
            self.actions.push("quit");
            Ok(())
        }
    }

    #[test]
    fn starts_hidden_and_toggles_both_directions() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        assert_eq!(controller.state(), LifecycleState::Hidden);
        assert_eq!(
            controller.handle(LifecycleCommand::Toggle, &mut window),
            Ok(LifecycleState::Visible)
        );
        assert_eq!(
            controller.handle(LifecycleCommand::Toggle, &mut window),
            Ok(LifecycleState::Hidden)
        );
        assert_eq!(window.actions, ["show", "hide"]);
    }

    #[test]
    fn native_close_hides_instead_of_exiting() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        controller
            .handle(LifecycleCommand::Show, &mut window)
            .unwrap();
        let state = controller.accept_window_close();

        assert_eq!(state, LifecycleState::Hidden);
        assert_eq!(window.actions, ["show"]);
    }

    #[test]
    fn explicit_close_command_uses_the_hide_port() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        controller
            .handle(LifecycleCommand::Show, &mut window)
            .unwrap();
        controller
            .handle(LifecycleCommand::WindowCloseRequested, &mut window)
            .unwrap();

        assert_eq!(controller.state(), LifecycleState::Hidden);
        assert_eq!(window.actions, ["show", "hide"]);
    }

    #[test]
    fn only_tray_exit_enters_exiting_state() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        let state = controller
            .handle(LifecycleCommand::ExitFromTray, &mut window)
            .unwrap();

        assert_eq!(state, LifecycleState::Exiting);
        assert_eq!(window.actions, ["quit"]);
    }

    #[test]
    fn state_does_not_change_when_platform_operation_fails() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow {
            fail_next: true,
            ..FakeWindow::default()
        };

        assert_eq!(
            controller.handle(LifecycleCommand::Show, &mut window),
            Err("show failed")
        );
        assert_eq!(controller.state(), LifecycleState::Hidden);
        assert!(window.actions.is_empty());
    }

    #[test]
    fn exiting_is_terminal_for_all_commands() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        controller
            .handle(LifecycleCommand::ExitFromTray, &mut window)
            .unwrap();
        assert_eq!(controller.state(), LifecycleState::Exiting);

        for command in [
            LifecycleCommand::Show,
            LifecycleCommand::Hide,
            LifecycleCommand::Toggle,
            LifecycleCommand::WindowCloseRequested,
            LifecycleCommand::ExitFromTray,
        ] {
            let result = controller.handle(command, &mut window).unwrap();
            assert_eq!(result, LifecycleState::Exiting);
        }

        assert_eq!(controller.state(), LifecycleState::Exiting);
        assert_eq!(window.actions, ["quit"]);
    }

    #[test]
    fn exit_failure_retains_retryable_state() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow {
            fail_next: true,
            ..FakeWindow::default()
        };

        assert_eq!(
            controller.handle(LifecycleCommand::ExitFromTray, &mut window),
            Err("quit failed")
        );
        assert_eq!(controller.state(), LifecycleState::Hidden);
        assert!(window.actions.is_empty());

        controller
            .handle(LifecycleCommand::ExitFromTray, &mut window)
            .unwrap();
        assert_eq!(controller.state(), LifecycleState::Exiting);
        assert_eq!(window.actions, ["quit"]);
    }

    #[test]
    fn native_close_after_exit_preserves_terminal_state() {
        let mut controller = LifecycleController::new();
        let mut window = FakeWindow::default();

        controller
            .handle(LifecycleCommand::ExitFromTray, &mut window)
            .unwrap();
        assert_eq!(controller.state(), LifecycleState::Exiting);

        // A queued native close request dispatched during shutdown must not
        // clear the terminal state.
        assert_eq!(controller.accept_window_close(), LifecycleState::Exiting);

        // A later Show must remain a no-op: only the original exit action runs.
        assert_eq!(
            controller.handle(LifecycleCommand::Show, &mut window),
            Ok(LifecycleState::Exiting)
        );
        assert_eq!(controller.state(), LifecycleState::Exiting);
        assert_eq!(window.actions, ["quit"]);
    }
}
