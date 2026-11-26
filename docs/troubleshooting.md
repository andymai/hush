# Troubleshooting

```bash
hush doctor                      # checks everything and prints the fix for each problem
hush status --full               # every component, with the CPU module and the device ggml selected
hush setup permissions --check   # keyboard and uinput access
hush status --devices            # microphones
hush test audio -d 3             # record and play back
hush -vv daemon start --foreground
```

If a notification says that the microphone did not start, Hush got no audio for five seconds. Hush opens the microphone again before the next recording. If the problem stays, run `hush test audio -d 3` to test the microphone, or set `audio.device` to a different one from `hush status --devices`.

If a notification says that the microphone stopped, the audio stopped for two seconds during a recording. Hush inserts what it heard until then, and opens the microphone again before the next recording.

If the first word of a recording gets cut off, the microphone probably sleeps between recordings. WirePlumber, the PipeWire session manager, suspends an idle microphone after 5 seconds. Some USB microphones then need most of a second to wake. The `Microphone wake` line of `hush doctor` shows whether your microphone sleeps and how long the last recordings waited for it. It warns only after a recording waited more than 200 ms.

`hush setup mic` writes one WirePlumber rule that keeps only that microphone awake. It supports WirePlumber 0.4 and 0.5, and touches no other file. WirePlumber then restarts, so all sound stops for about a second. No app records while the microphone is awake, so the privacy indicator stays off. `hush setup mic --undo` removes the rule.

After such a slow recording, the Speech page of `hush settings` shows the same warning, with a button for each command. Hush does not keep a Bluetooth microphone awake, because an open headset link lowers the sound quality.

If Hush does not start at login on a package install, run `systemctl --user status hush.service`. `hush install --autostart` enables the unit, and `journalctl --user -u hush.service` shows why a start failed. On Sway, Hyprland, or niri started from a TTY, neither the unit nor an autostart entry runs at login. Add the line from [commands.md](commands.md) to the compositor's config.

If `hush setup permissions` cannot run where you are (a container, an SSH session), `hush setup permissions --print` prints the udev rule to apply as root.

In the first three minutes after boot, Hush waits up to five seconds for the graphics driver. If the driver is not ready then, the speech model runs on the CPU until you run `hush daemon restart`. When a take fails on the GPU, Hush runs it again on the CPU. Hush then keeps that model family (Whisper or Parakeet) on the CPU for that GPU until the driver version changes. When the GPU runs out of memory for a take, only that take runs again on the CPU, and the next take uses the GPU. `hush doctor` shows a `GPU failure` line with the command that tries the GPU again.

Hush runs on any x86-64 CPU. Its speech engine has a CPU module for each x86 level, and at start ggml loads the module that fits your CPU. The `Backends` line of `hush doctor` and `hush status --full` names that module and the GPU backend. The modules live next to `libtranscribe`: in `/usr/lib/hush` for a package, or in `lib/hush` next to the `bin` folder of the tarball. The binary looks for them there, so a copy of the binary alone does not start. From source, install with `make install`, not `cargo install`, as [INSTALL.md](../INSTALL.md#libraries-and-cpu) describes.
