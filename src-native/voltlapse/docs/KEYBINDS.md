# Default keybindings

All of these are rebindable in **Options → Controls → Volt Lapse**. The config
stores the vanilla key name, so a binding that becomes invalid (a keyboard
layout change, a controller swap) is repaired on load rather than lost.

| Action | Config id | Default |
| --- | --- | --- |
| Start / stop recording | `start_stop_recording` | `F6` |
| Pause / resume recording | `pause_resume_recording` | `F7` |
| Open the replay viewer | `open_viewer` | `F8` |
| Open the camera editor | `open_camera_editor` | `F9` |
| Toggle free camera | `toggle_free_cam` | `F10` |
| Add a camera keyframe | `add_keyframe` | `K` |
| Screenshot (into the replay) | `screenshot` | `F2` |
| Toggle the HUD | `toggle_hud` | `F1` |
| Playback speed up | `speed_up` | `=` |
| Playback speed down | `speed_down` | `-` |
| Play / pause playback | `play_pause` | `Space` |
| Seek forward | `seek_forward` | `→` |
| Seek backward | `seek_backward` | `←` |
| Toggle looping | `toggle_loop` | `L` |
| Render the active camera path | `render_last_path` | `F11` |

Inside the viewer and the camera editor the same keys act on the open
replay/path; the panels also respond to the mouse. `F1` hides the overlay
without stopping a recording — useful for clean footage.

## Conflicts

The engine detects a binding used by two actions and by vanilla
(`F2`, `F1`, `Space`, the arrows) and reports it in the keybinding screen.
Nothing is unbound automatically: you get the list and pick.

## Commands

Every keybind has a command equivalent, which matters on touch devices and in
command blocks:

```text
/voltlapse record start | stop | pause | resume | save | stats
/voltlapse play [replay] [at 90] [speed 2]
/voltlapse render start | queue | cancel | status
/voltlapse keyframe add | delete | clear | list
/voltlapse bookmark add | remove | list | next | previous
/voltlapse config <section> [key] [value]
/voltlapse list | info | export | help
/voltlapse share host | join | leave | status
```
