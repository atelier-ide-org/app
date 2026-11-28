use super::*;

/// A terminal tab sent to a window of its own leaves the panel, runs there with its shell, and
/// comes back as the active tab of the panel, the same shell still running.
#[test]
fn a_terminal_goes_to_a_window_and_comes_back() {
    let mut ide = super::super::tests::ide();
    ide.start_terminal(None);
    ide.start_terminal(None);
    assert_eq!(ide.terminals.len(), 2);
    let id = ide.terminals[0].active;
    ide.detach_terminal(0, None);
    assert_eq!(ide.terminals.len(), 1);
    assert_eq!(ide.terminal_windows.len(), 1);
    assert_eq!(ide.terminal_windows[0].tab.active, id);
    assert!(ide.active_terminal < ide.terminals.len());
    // Split in its window: a second shell beside the first.
    ide.window_action(0, WindowAction::Split(false));
    assert_eq!(ide.terminal_windows[0].tab.count(), 2);
    ide.attach_terminal(0);
    assert!(ide.terminal_windows.is_empty());
    assert_eq!(ide.terminals.len(), 2);
    assert_eq!(ide.active_terminal, 1);
    assert_eq!(ide.terminals[1].count(), 2);
    assert!(ide.terminals[1].root.leaves().iter().any(|p| p.id == id));
    assert!(ide.show_terminal);
}

/// The last tab of the panel sent to a window: the panel closes, nothing points past the tabs.
#[test]
fn detaching_the_last_terminal_closes_the_panel() {
    let mut ide = super::super::tests::ide();
    ide.set_terminal(true);
    ide.start_terminal(None);
    ide.detach_terminal(0, None);
    assert!(ide.terminals.is_empty());
    assert!(!ide.show_terminal);
    assert_eq!(ide.active_terminal, 0);
    // Closing its window ends its shells.
    ide.window_action(0, WindowAction::Close);
    assert!(ide.terminal_windows.is_empty());
}
