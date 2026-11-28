# GeoPolit perf tools (Playwright, Chromium)

Serve `geo/src/main/assets` on a port (e.g. `python3 -m http.server 8765`), then:

- `node sim_bench.js 8765 600` — pure simulation benchmark: N divisions on 5 fronts, offensive plans, 300 hourly ticks; per-system ms/tick and a hash of the control grid.
- `node determinism.js` — seeded scenario with fake clock; prints hash of map control + division positions. Must be identical before/after any optimization.
- `node stress_realtime.js 600 20` — real game loop at max speed with per-function profiling (render numbers are meaningless in headless software GL).

In game: «Отладка» button shows the built-in profiler (ms per frame per system, FPS, ticks/s).
