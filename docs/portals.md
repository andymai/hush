# Flatpak and desktop portals

A Flatpak sandbox gives Hush no access to `/dev/uinput` or `/dev/input`. In a sandbox, Hush uses the desktop portals:

- The RemoteDesktop portal types the text. At the first dictation, the desktop asks you to allow keyboard access. Hush keeps the answer in `remote-desktop-token` in its data folder, a file that only you can read, so the desktop does not ask again. On GNOME, the dialog's Allow Remote Interaction switch starts off: switch it on, then press Share.
- The GlobalShortcuts portal reads the hotkeys. Hush asks for one shortcut for each action and proposes Meta+Space for dictation. You set the other keys in the shortcut settings of the desktop. `hush shortcuts` shows the keys, and `hush shortcuts --configure` opens the shortcut settings.
- The Background portal starts Hush at login. Run `hush install --autostart`, or turn on the switch in `hush settings`.

Hold to talk, the double-tap lock, and toggle mode work as with evdev. The compositor drops a character that the active layout does not have, so Hush types only the characters that the layout surely has, and pastes other text:

- In a sandbox, Hush reads the keymap that the compositor sends to each Wayland app. If the keymap has one layout, Hush types the characters on the first two levels of that layout, with no dead key.
- If the keymap in a sandbox has two or more layouts, Hush cannot tell which layout is active. Hush also cannot read a keymap on an X11 session. Then it types letters, digits, spaces, and `. , ; : ! ? - ( )`, and pastes other text, such as quotes, accents, and emoji. On KDE Plasma, it types only letters and spaces, and pastes a take with a digit or a punctuation mark.
- Outside a sandbox, Hush reads the layout settings and types the characters on the first two levels of the active layout, with no dead key.

For a character on the second level of its key, such as a capital letter, Hush holds Shift itself. KWin does not add Shift correctly for such a character, so the character comes out without Shift, and Shift stays down after it.

The paste chord is Shift+Insert, because Shift and Insert have the same place on every layout. Each take opens a RemoteDesktop session and closes it after the text, so GNOME shows its sharing indicator only during the take. In a sandbox, Hush has no update check, because Flatpak updates it.

Portal mode needs KDE Plasma 6 or GNOME 48, or a later version. xdg-desktop-portal-wlr, -gtk, and -hyprland have no RemoteDesktop portal. On Sway, Hyprland, and other wlroots compositors, install Hush from a package instead. `hush doctor` shows which portals the desktop has.

In the Flatpak, the recording pill uses a layer-shell surface when the compositor gives one to a sandboxed app. Otherwise it draws in an X11 window through Xwayland, so the manifest asks for X11 as well as Wayland.

In a sandbox, Hush keeps each API key that you save in `hush settings` in its own `.env` file, `~/.var/app/io.github.andymai.hush/config/hush/.env`, which only you can read. Hush does not use the keyring through the Secret portal. The unlock prompt of the keyring can take the keyboard focus while Hush types, and the text would then go into the password field.

If you refuse keyboard access, Hush puts the text on the clipboard and does not ask again. Run `hush daemon restart`, and the desktop asks at the next dictation. To try portal mode outside a sandbox, set `backend = "portal"` under `[input]` and restart the daemon.
