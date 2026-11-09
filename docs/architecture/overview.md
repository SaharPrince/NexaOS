# NexaOS Architecture — Initial Direction

NexaOS is planned as a Linux-based distribution. The Linux kernel and hardware-driver ecosystem provide the host foundation; Nexa's original work is a user-mode Windows compatibility engine and its integration.

## Nexa Compatibility Engine (NCE)

Planned components include PE inspection/parsing, image loading, Windows x64 ABI bridging, NT-style user-mode services, Win32 APIs, registry and COM behavior, networking, audio, graphics translation, and per-application security boundaries. These are work areas, not claims of implemented functionality.

## Compatibility boundaries

A user-mode compatibility engine can implement user-mode behavior. It cannot transparently replace arbitrary Windows kernel drivers. Multiplayer anti-cheat support is limited to configurations officially supported by the publisher and provider.

## NCE-001 boundary

NCE-001 is a read-only metadata inspector. It must not map executable sections into executable memory, resolve imports, load DLLs, or transfer control to an entry point.
