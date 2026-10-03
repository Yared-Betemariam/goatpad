# Local patches

This is winit 0.30.13 with one Windows fix in `src/platform_impl/windows/event_loop.rs`.
During an interactive move, `WM_DPICHANGED` now applies Windows' suggested rectangle directly.
The previous monitor-correction loop could move the window back to the old monitor and trigger
repeated DPI changes while dragging across displays with different scaling.
