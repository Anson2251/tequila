# Supported Registry Keys

All keys read and written by the Registry Editor are under `HKEY_CURRENT_USER` (HKCU).
The `HKEY_CURRENT_USER\` prefix is implicit — the editor methods accept bare paths like
`Software\Wine\Direct3D`.

If a key or value does not exist, create it. All values are strings (REG_SZ) unless stated otherwise.

---

## General Tab

### Windows Version

| Full path | Value | Type | Values |
|-----------|-------|------|--------|
| `HKCU\Software\Wine` | `Version` | REG_SZ | `win10`, `win81`, `win8`, `win7`, `win2008`, `vista`, `win2003`, `winxp`, `win2k`, `nt40`, `winme`, `win98`, `win95`, `win31` |

Sets the version of Windows Wine will report to applications.

### Audio Driver

| Full path | Value | Type | Values |
|-----------|-------|------|--------|
| `HKCU\Software\Wine\Drivers\Audio` | `(default)` | REG_SZ | `pulse`, `alsa`, `oss`, `coreaudio`, `""` (disabled) |

Which audio backend to use. Given a comma-separated list of drivers, Wine will attempt to make
the most appropriate choice. Set to empty string to disable audio entirely.

### DPI

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Control Panel\Desktop` | `LogPixels` | REG_DWORD | Minimum 96 |

Sets current DPI (font size). Some dialogs resize themselves according to this value.
Default: 96 (decimal).

### Debug

Not exposed in the notebook UI directly, but useful for troubleshooting.

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Debug` | `RelayExclude` | REG_SZ | Exclude calls from +relay debug log; e.g. `ntdll.RtlEnterCriticalSection;ntdll.RtlLeaveCriticalSection` |
| `HKCU\Software\Wine\Debug` | `RelayFromExclude` | REG_SZ | Exclude calls made from listed DLLs; e.g. `kernel32` omits all calls from kernel32.dll |
| `HKCU\Software\Wine\Debug` | `SpyExclude` | REG_SZ | Exclude messages from +message debug log; e.g. `WM_TIMER;WM_MOUSEMOVE;WM_PAINT` |
| `HKCU\Software\Wine\Debug` | `SpyInclude` | REG_SZ | Only include listed messages; e.g. `WM_CREATE` |

---

## Graphics Tab

### Direct3D

| Full path | Value | Type | Values |
|-----------|-------|------|--------|
| `HKCU\Software\Wine\Direct3D` | `renderer` | REG_SZ | `gl`, `vulkan`, `gdi` / `no3d` |
| `HKCU\Software\Wine\Direct3D` | `csmt` | REG_DWORD | `1` (on, default), `0` (off) |
| `HKCU\Software\Wine\Direct3D` | `OffscreenRenderingMode` | REG_SZ | `fbo` (default), `backbuffer` |
| `HKCU\Software\Wine\Direct3D` | `VideoMemorySize` | REG_DWORD | 1–16384 (MB) |

- **renderer** — Select what backend to use for wined3d. `gdi` (alias `no3d`) mostly exists for legacy or test reasons.
- **csmt** — Multi-threaded command stream. Bitmask since Wine 6.0-rc1: `0x1` enables command stream, `0x3` also forces serialisation of OpenGL/Vulkan commands between multiple command streams (useful for MS Office 2013+ on buggy drivers like Nouveau).
- **OffscreenRenderingMode** — `fbo` uses framebuffer objects; `backbuffer` renders in the backbuffer.
- **VideoMemorySize** — Override reported video memory in megabytes. By default Wine queries `GLX_MESA_query_renderer` or estimates from PCI IDs.

Shader model limits (defined in code, not exposed in UI):

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelVS` | REG_DWORD | Max vertex shader model; `0` disables |
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelPS` | REG_DWORD | Max pixel shader model; `0` disables |
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelGS` | REG_DWORD | Max geometry shader model; `0` disables |
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelHS` | REG_DWORD | Max hull shader model; `0` disables |
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelDS` | REG_DWORD | Max domain shader model; `0` disables |
| `HKCU\Software\Wine\Direct3D` | `MaxShaderModelCS` | REG_DWORD | Max compute shader model; `0` disables |

Additional Direct3D keys (advanced, not exposed in UI):

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Direct3D` | `CheckFloatConstants` | REG_DWORD | Range check float constants in d3d9 shaders; workaround for geometry glitch bugs like [#34052](https://bugs.winehq.org/show_bug.cgi?id=34052). Default disabled. |
| `HKCU\Software\Wine\Direct3D` | `MaxVersionGL` | REG_DWORD | Max OpenGL version to request. Defaults to 4.4 since Wine 3.9. Set to `30002` (hex) or greater for core profile context. |
| `HKCU\Software\Wine\Direct3D` | `MultisampleTextures` | REG_DWORD | `1` (default) enable multisample textures; `0` uses renderbuffers. |
| `HKCU\Software\Wine\Direct3D` | `SampleCount` | REG_DWORD | Override swapchain sample count. Force-enables multisampling for apps that don't support it. |
| `HKCU\Software\Wine\Direct3D` | `shader_backend` | REG_SZ | `glsl`, `arb`, `none`. Auto-detected if unset (prefers glsl). |
| `HKCU\Software\Wine\Direct3D` | `UseGLSL` | REG_SZ | `disabled` to disable GLSL for shaders. Only for debugging. |
| `HKCU\Software\Wine\Direct3D` | `strict_shader_math` | REG_DWORD | `1` to disable NVIDIA's aggressive GLSL optimizations; workaround for incorrect rendering like [#35207](https://bugs.winehq.org/show_bug.cgi?id=35207). Default `0`. |
| `HKCU\Software\Wine\Direct3D` | `VideoPciDeviceID` | REG_DWORD | Override PCI device ID of the graphics card. |
| `HKCU\Software\Wine\Direct3D` | `VideoPciVendorID` | REG_DWORD | Override PCI vendor ID of the graphics card. |
| `HKCU\Software\Wine\Direct3D` | `WineLogo` | REG_SZ | Path to image file to use as Wine logo. |

### Virtual Desktop (Linux/X11 only — hidden on macOS)

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Explorer` | `Desktop` | REG_SZ | `"Default"` to enable, deleted to disable |
| `HKCU\Software\Wine\Explorer\Desktops` | `Default` | REG_SZ | `"1024x768"` format; deleted when disabled |
| `HKCU\Software\Wine\Explorer` | `ShowSystray` | REG_DWORD | `1` (show, default), `0` (hide); not in UI tab |

The `Desktop` key controls the title of the default virtual desktop window. The special value
`"shell"` also displays the shell (Start menu, taskbar, system tray) unless overridden by
`EnableShell`.

Under `Desktops`, you can create named subkeys for different desktop sizes — the most common is
`Default`. Size strings use `"WxH"` format (e.g. `"1400x1050"`). Defaults to 800x600.

The shell in virtual desktop can be controlled per-desktop via:
- `HKCU\Software\Wine\Explorer\Desktops\<name>\EnableShell` (REG_DWORD; default `0x1` for
  the desktop named `"shell"`, `0x0` for all others).

---

## Platform Tab

### Mac Driver (macOS only)

Boolean values use Wine's `IS_OPTION_TRUE` macro: `Y`/`y`/`T`/`t`/`1` = true, else false.
The editor writes uppercase `Y` / `N`.

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Mac Driver` | `AllowVerticalSync` | REG_SZ | `Y`/`N`; set to `N` to disable vsync |
| `HKCU\Software\Wine\Mac Driver` | `CaptureDisplaysForFullscreen` | REG_SZ | `Y`/`N`; capture displays (disables hot corners) when full-screen even without resolution change |
| `HKCU\Software\Wine\Mac Driver` | `UsePreciseScrolling` | REG_SZ | `Y`/`N`; set to `N` to emulate "clicky" mouse wheel for apps that scroll too far with precision events |
| `HKCU\Software\Wine\Mac Driver` | `RetinaMode` | REG_SZ | `Y`/`N`; expose full Retina (HiDPI) resolutions |
| `HKCU\Software\Wine\Mac Driver` | `LeftOptionIsAlt` | REG_SZ | `Y`/`N`; left ⌥ Option key behaves as Alt |
| `HKCU\Software\Wine\Mac Driver` | `RightOptionIsAlt` | REG_SZ | `Y`/`N`; right ⌥ Option key behaves as Alt |
| `HKCU\Software\Wine\Mac Driver` | `LeftCommandIsCtrl` | REG_SZ | `Y`/`N`; left ⌘ Command key behaves as Ctrl |
| `HKCU\Software\Wine\Mac Driver` | `RightCommandIsCtrl` | REG_SZ | `Y`/`N`; right ⌘ Command key behaves as Ctrl |
| `HKCU\Software\Wine\Mac Driver` | `WindowsFloatWhenInactive` | REG_SZ | `none`, `all`, `nonfullscreen` (default); controls TOPMOST window behavior when Wine is in background |

### X11 Driver

Boolean values: present/absent/`Y` = true (opt-in), `N` = false (opt-out).
The editor writes uppercase `Y` / `N`.

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\X11 Driver` | `Decorated` | REG_SZ | `N` to disallow window manager decorations |
| `HKCU\Software\Wine\X11 Driver` | `Managed` | REG_SZ | `N` to disallow window manager control |
| `HKCU\Software\Wine\X11 Driver` | `GrabPointer` | REG_SZ | `N` to disallow mouse capture |
| `HKCU\Software\Wine\X11 Driver` | `GrabFullscreen` | REG_SZ | `Y` to force full-screen windows to capture mouse |
| `HKCU\Software\Wine\X11 Driver` | `ClientSideGraphics` | REG_SZ | `N` to disable DIB engine client-side rendering |
| `HKCU\Software\Wine\X11 Driver` | `ClientSideWithRender` | REG_SZ | `N` to disable Render extension client-side fonts |
| `HKCU\Software\Wine\X11 Driver` | `ClientSideAntiAliasWithRender` | REG_SZ | `N` to disable font anti-aliasing when X-Render is present |
| `HKCU\Software\Wine\X11 Driver` | `ClientSideAntiAliasWithCore` | REG_SZ | `N` to disable font anti-aliasing when X-Render is absent |
| `HKCU\Software\Wine\X11 Driver` | `UseXRandR` | REG_SZ | `N` to prevent Wine from switching resolution via XRandR |
| `HKCU\Software\Wine\X11 Driver` | `UseXVidMode` | REG_SZ | `Y` to allow resolution switching via XVidMode |
| `HKCU\Software\Wine\X11 Driver` | `UseEGL` | REG_SZ | `N` to use GLX instead of EGL for OpenGL |

---

## Keys Defined in Code (No UI Tab)

These are available through the `RegEditor` trait API but not exposed in the notebook tabs.

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Drivers\Graphics` | `(default)` | REG_SZ | `x11`, `mac`, `null` (virtual headless) |
| `HKCU\Software\Wine\DirectInput` | `MouseWarpOverride` | REG_SZ | `enable` (default), `disable`, `force` |
| `HKCU\Software\Wine\DirectInput` | `DefaultDeadZone` | REG_DWORD | 0–10000; percentage of joystick axis treated as centered |
| `HKCU\Software\Wine\Fonts\Replacements` | `<font name>` | REG_SZ | Dynamic: value name = original font, value = replacement |
| `HKCU\Software\Wine\Fonts\ExternalFonts` | `<font name>` | REG_SZ | Dynamic: list of font names whose values are the actual font name |
| `HKCU\Software\Wine\DllOverrides` | `<dll name>` | REG_SZ | Dynamic: `native`, `builtin`, `native,builtin`, `builtin,native`, `""` (disabled) |
| `HKCU\Software\Wine\AppDefaults\<app.exe>` | `DllOverrides`, `Direct3D\renderer`, etc. | mixed | Per-application override settings |

---

## Additional Wine Registry Keys

Keys not currently exposed via the editor UI, but commonly useful for troubleshooting.

### DirectInput

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\DirectInput` | `MouseWarpOverride` | REG_SZ | `enable` (default), `disable`, `force` — warp pointer behavior when mouse exclusively acquired |
| `HKCU\Software\Wine\DirectInput` | `DefaultDeadZone` | REG_DWORD | 0–10000; percentage of joystick axis treated as centered |
| `HKCU\Software\Wine\DirectInput` | `<joystick name>` | REG_SZ | Axis mapping: `X,Y,Rz,Slider1,POV1` — comma-separated axis types per joystick |

### DirectSound

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\DirectSound` | `HelBuflen` | REG_DWORD | Hardware emulation buffer length; default 65536 |

### MSHTML (Gecko)

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\MSHTML` | `GeckoPath` | REG_SZ | Path to Wine Gecko engine; e.g. `c:\Program Files\wine_gecko` |
| `HKCU\Software\Wine\MSHTML` | `GeckoUrl` | REG_SZ | URL for Gecko downloads; default `https://source.winehq.org/winegecko.php`. Can be `file:///Z:/path/to/wine_gecko.cab` for offline. |
| `HKCU\Software\Wine\MSHTML\CompatMode` | `MaxCompatMode` | REG_SZ | Max IE version to expose to web pages. Subkeys with host names (`.winehq.org`) set per-site compat mode. |

### OpenGL

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\OpenGL` | `DisabledExtensions` | REG_SZ | Space-separated list of OpenGL extensions to hide from applications |
| `HKCU\Software\Wine\OpenGL` | `EnabledExtensions` | REG_SZ | Space-separated list of the _only_ OpenGL extensions to report |

### Network

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Network` | `UseDnsComputerName` | REG_SZ | Set to `N` for a persistent NetBIOS ComputerName (set via HKLM) |

### WineBrowser

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\WineBrowser` | `Browsers` | REG_SZ | Comma-separated browser list for winebrowser; default `xdg-open,firefox,konqueror,...` |
| `HKCU\Software\Wine\WineBrowser` | `Mailers` | REG_SZ | Comma-separated mail client list; default `xdg-email,mozilla-thunderbird,thunderbird,evolution` |

### WineDbg

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\WineDbg` | `BreakOnFirstChance` | REG_DWORD | `1` (default) to break on first-chance exceptions; `0` lets apps handle them first |
| `HKCU\Software\Wine\WineDbg` | `ShowCrashDialog` | REG_DWORD | `1` (default) to show GUI crash dialog; `0` to disable |

### Drivers

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Drivers\winealsa.drv` | `ALSAOutputDevices` | REG_MULTI_SZ | Auxiliary ALSA output devices not enumerated by hardware detection |
| `HKCU\Software\Wine\Drivers\winealsa.drv` | `ALSAInputDevices` | REG_MULTI_SZ | Auxiliary ALSA input devices not enumerated by hardware detection |

### Printing

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Printing\PPD Files` | `<printer name>` | REG_SZ | Full Unix path to PPD file per printer |
| `HKCU\Software\Wine\Printing\PPD Files` | `generic` | REG_SZ | Default PPD file when all else fails |
| `HKCU\Software\Wine\Printing\Spooler` | `<port name>` | REG_SZ | Redirect printer port to Unix file or pipe (`\|command`) |

### Fonts

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Fonts\Replacements` | `<font name>` | REG_SZ | Replace one font with another. e.g. `Wingdings` = `Winedings` |
| `HKCU\Software\Wine\Fonts\ExternalFonts` | `<font name>` | REG_SZ | Register additional font names pointing to actual font files |

### Virtual Desktop Shell

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKCU\Software\Wine\Explorer\Desktops\<name>` | `EnableShell` | REG_DWORD | `1` to show taskbar/start menu in virtual desktop; only enabled by default for desktop named `"shell"` |

---

## HKEY_LOCAL_MACHINE (HKLM)

Keys under `HKEY_LOCAL_MACHINE` are read by Wine but typically managed automatically. They are
listed here for reference when manual intervention is needed.

### DirectDraw

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\Software\Microsoft\DirectDraw` | `ForceRefreshRate` | REG_DWORD | Force refresh rate (Hz) for DirectX games |

### Internet Explorer

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\Software\Microsoft\Internet Explorer` | `Version` | REG_SZ | IE version string; `6.0.2800.1106` for IE6SP1 |
| `HKLM\Software\Microsoft\Internet Explorer` | `W2kVersion` | REG_SZ | Windows 2000 IE version |
| `HKLM\Software\Microsoft\Internet Explorer` | `Build` | REG_SZ | IE build number; `62800.1106` for IE6SP1 |

### Font Substitutes

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\Software\Microsoft\Windows NT\CurrentVersion\FontSubstitutes` | `<font name>` | REG_SZ | e.g. `Tahoma` = `Arial` substitutes Tahoma with Arial |

### AeDebug

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\Software\Microsoft\Windows NT\CurrentVersion\AeDebug` | `Debugger` | REG_SZ | Debugger command on unhandled exception; default `winedbg --auto %ld %ld` |

### Wine Ports

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\Software\Wine\Ports` | `<Win32 path>` | REG_SZ | Map Unix serial/parallel ports to Win32; e.g. `COM3` = `/dev/ttyS5` |

### System Environment

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` | `PATH` | REG_SZ | Default `c:\windows\system32;c:\windows` |
| `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` | `ProgramFiles` | REG_SZ | Default `C:\Program Files` |
| `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` | `TEMP` / `TMP` | REG_SZ | Default `c:\windows\temp` |
| `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` | `windir` | REG_SZ | Default `c:\windows` |
| `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` | `winsysdir` | REG_SZ | Default `c:\windows\system32` |
| `HKLM\System\CurrentControlSet\Control\Session Manager` | `GlobalFlag` | REG_DWORD | Internal diagnostics flags (heap checking, etc.) |

### Computer Name

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\System\CurrentControlSet\Control\ComputerName\ComputerName` | `ComputerName` | REG_SZ | Persistent NetBIOS name; auto-set from hostname unless `UseDnsComputerName=N` |

### HID / Bus

| Full path | Value | Type | Notes |
|-----------|-------|------|-------|
| `HKLM\System\CurrentControlSet\Services\WineBus` | `DisableHidraw` | REG_DWORD | `1` to disable hidraw HID discovery |
| `HKLM\System\CurrentControlSet\Services\WineBus` | `EnableHidraw` | REG_SZ | Comma-separated `VID:PID` list to enable hidraw for specific devices |
| `HKLM\System\CurrentControlSet\Services\WineBus` | `DisableInput` | REG_DWORD | `1` to disable evdev HID discovery |
| `HKLM\System\CurrentControlSet\Services\WineBus` | `Enable SDL` | REG_DWORD | `1` (default) enable SDL for HID devices; `0` disable |
| `HKLM\System\CurrentControlSet\Services\WineBus` | `Map Controllers` | REG_DWORD | `1` (default) convert SDL controllers to XInput-compatible gamepads |
| `HKLM\System\CurrentControlSet\Services\WineBus\Map` | `<any name>` | REG_SZ | SDL game controller mapping string (passed to `SDL_GameControllerAddMapping()`) |
| `HKLM\System\CurrentControlSet\Services\WineBus\Devices` | `<VID>[/PID]\HidRaw` | REG_DWORD | `1` enable hidraw for specific vendor/product |

---

## Reading and Writing

The editor reads from the prefix's real `.reg` files (`system.reg`, `userdef.reg`, `user.reg`)
via the `regashii` library. Writes go back to `user.reg`.

An SQLite cache (`PrefixStore`) mirrors the parsed values for fast UI loading across sessions.
Cache entries are stored under section/key paths matching the registry paths listed above
(without the `HKCU\` prefix). On cache miss, `.reg` files are re-parsed.
