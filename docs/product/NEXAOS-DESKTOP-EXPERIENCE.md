# NexaOS Desktop Experience

## Product promise

**A complete, beautiful, customizable Windows-familiar desktop — engineered to use resources intelligently.**

For NexaOS, lightweight describes implementation efficiency, not a reduced feature set. The product must not become visually bare, difficult to personalize, or functionally incomplete merely to lower an idle-memory number.

Users should be able to choose how their computer looks and works without being forced into one layout or workflow. The default experience should feel familiar to someone coming from Windows, while remaining coherent, accessible, responsive, and safe.

## Non-negotiable principles

1. **Feature completeness over artificial minimalism.** Do not remove useful desktop capabilities just to make the package list or idle footprint smaller. Prefer efficient implementations, modular components, and on-demand activation.
2. **Visual quality is part of the product.** Provide a considered wallpaper, coherent iconography, typography, spacing, panel, menus, window controls, notifications, and polished application surfaces.
3. **Customization is a first-class capability.** Users should be able to personalize layout, panel/taskbar, start/application menu, icons, themes, accent colors, wallpapers, fonts, shortcuts, window behavior, workspaces, and startup preferences.
4. **Smoothness must be measured.** Optimize input latency, frame pacing, animation behavior, application startup, file browsing, and responsiveness under memory pressure. Effects should adapt to hardware and respect reduced-motion preferences.
5. **Resource efficiency comes from engineering.** Avoid duplicate services, unnecessary startup work, unbounded scans, wasteful polling, and heavyweight always-on processes. Load optional functionality when needed and release resources when idle.
6. **Customization must not create security loopholes.** Preferences are per-user and validated. Themes, widgets, extensions, and presets cannot disable security boundaries, updates, recovery, permissions, or compatibility safeguards.
7. **Familiarity without false promises.** Use recognizable desktop patterns and workflows. Do not claim every Windows application, driver, game, or anti-cheat system is supported until it is individually verified.

## Experience areas

### Desktop and shell
- A polished desktop with user-selectable wallpapers, icons, desktop folders, context menus, and layout options.
- A configurable taskbar/panel with app launcher, pinned and running applications, task switching, system tray, clock, notifications, and quick settings.
- Familiar window operations: move, resize, maximize, minimize, snap/tiling, focus behavior, and multiple workspaces.
- Search and application discovery that remain responsive and do not continuously scan the entire disk.

### Applications and files
- A complete, dependable file manager with local storage, removable volumes, network locations when supported, tabs, search, previews, and safe file operations.
- Useful built-in applications and settings surfaced consistently in the application menu.
- Clear application install, update, launch, permission, and failure experiences.
- Windows application integration is a staged goal, not an enabled capability in the current preview. It remains subject to the separate runtime and security milestones.

### Personalization
- First-run choices should offer a familiar desktop preset and other layouts, but presets are starting points rather than locked editions.
- Let users change panel position, size, visibility, pinned apps, menu layout, desktop icons, themes, icon packs, accent colors, fonts, wallpaper, cursor, shortcuts, workspace behavior, and supported effects.
- Provide preview, apply, undo/reset, and safe fallback for customization changes.
- Keep personal preferences per-user; ordinary appearance changes must not require administrator access.
- Validate settings, version configuration, preserve recoverable preferences, and recover safely from broken themes or panel plugins.
- Respect accessibility, keyboard-only use, display scaling, high contrast, and reduced motion.

## Performance and resource principles

- Establish documented performance budgets for supported hardware tiers, then measure real behavior rather than optimizing for a single idle-RAM screenshot.
- Measure cold boot, login-to-interactive-desktop, idle and active RAM, idle and active CPU, storage footprint, app launch, file-manager navigation, animation frame pacing, and recovery after memory pressure.
- Keep the core shell responsive when optional services or compatibility components are absent or unavailable.
- Prefer event-driven services and bounded caches over polling and unbounded background indexing.
- Make optional effects and widgets hardware-aware and user-controllable. The default should look polished without requiring expensive effects.
- Avoid sacrificing useful functionality to meet a budget; profile the bottleneck and improve the implementation or revise the budget transparently.

## Security and reliability requirements

- Run the desktop and user applications as the logged-in user by default.
- No automatic privilege elevation, shell-string command construction, or execution of user-selected files through UI metadata.
- Keep system policy separate from user appearance and workflow preferences.
- Treat third-party themes, icons, widgets, plugins, filenames, and metadata as untrusted input.
- A broken customization component must not prevent login; provide a known-good session and reset path.
- A user application or desktop plugin crash must not take down the whole session where isolation is practical.
- Preserve a recovery path to a text console or known-good desktop configuration.

## Delivery stages

1. **Reliable visual baseline:** working session, panel, application menu, icon theme, wallpaper, desktop icons, file manager, settings, notifications, and clean default appearance.
2. **Complete daily workflow:** reliable local/removable/network storage support, application discovery, window management, search, and accessible settings.
3. **Customization center:** previewable themes, layouts, panel settings, wallpaper, icons, shortcuts, and reversible per-user profiles.
4. **Performance engineering:** repeatable measurements on low-memory and mainstream hardware; optimize measured bottlenecks without feature removal.
5. **Application compatibility:** integrate Windows application support only after the separate reviewed runtime, isolation, and vendor-policy milestones pass.

## Current preview truth

The current NexaOS preview is Debian 13 with the distribution-packaged LXQt desktop and a small NexaOS Welcome application. The screenshot from the first VMware test showed that the LXQt panel and session were running, but the desktop appeared unfinished: black wallpaper, missing visible icon artwork, and unsupported Computer/Network locations. These are real integration defects, not the intended product design.

The initial desktop repair explicitly includes GVFS backends, GVFS FUSE integration, and Papirus icons because Debian's PCManFM-Qt package recommends these components and the NexaOS build disables APT recommendations globally. A default desktop wallpaper and PCManFM-Qt profile are being added to make the first boot look intentional.

Passing an ISO build or a nonblank QEMU framebuffer is not proof of a complete desktop. Release gates must include interactive VM checks and documented low-end hardware measurements. The current preview does not launch Windows applications.
