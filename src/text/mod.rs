/// Text insertion system with universal application compatibility
///
/// This module provides hardware-level text insertion using Linux UInput,
/// enabling typing into ANY application including VMs, password fields,
/// games, and secure contexts.
///
/// # Components
///
/// - [`TextInserter`] - High-level text insertion with X11 and UInput backends
/// - [`UinputKeyboard`] - Low-level kernel keyboard emulation
/// - Setup utilities - UInput configuration and diagnostics
///
/// # Insertion Methods
///
/// 1. **UInput** (recommended) - Kernel-level key events, works everywhere
/// 2. **X11** - X11 protocol, faster but limited to X11 apps
/// 3. **Clipboard** - Fallback method, requires manual paste
///
/// # Examples
///
/// ```no_run
/// use hush::text::{TextInserter, InsertionMethod};
///
/// // Create text inserter with UInput
/// let inserter = TextInserter::new(InsertionMethod::UInput)
///     .expect("Failed to create inserter");
///
/// // Insert text at current cursor
/// inserter.insert_text("Hello, world!")
///     .expect("Failed to insert text");
/// ```
///
/// # Setup Requirements
///
/// UInput requires proper permissions:
///
/// ```bash
/// # Load kernel module
/// sudo modprobe uinput
///
/// # Set permissions (temporary)
/// sudo chmod 666 /dev/uinput
///
/// # Or use automated setup
/// ./hush setup uinput --auto-fix
/// ```
///
/// # Compatibility
///
/// With UInput, text insertion works in:
/// - ✅ All desktop applications (editors, browsers, terminals)
/// - ✅ Password fields and secure inputs
/// - ✅ Virtual machines (VMware, VirtualBox, QEMU)
/// - ✅ Games and fullscreen applications
/// - ✅ SSH sessions and remote terminals
/// - ✅ X11 and Wayland applications

#[cfg(target_os = "linux")]
pub mod insertion;
#[cfg(target_os = "linux")]
pub mod uinput_keyboard;

#[cfg(target_os = "linux")]
pub use insertion::{
    check_dependencies, diagnose_uinput_issues, print_uinput_setup_guidance, InsertionMethod,
    TextInserter, WindowInfo,
};
#[cfg(target_os = "linux")]
pub use uinput_keyboard::{check_uinput_availability, UinputKeyboard};
