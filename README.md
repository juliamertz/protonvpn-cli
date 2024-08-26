# Work in progress

## Features
- [x] Filter by speed, load and exit country
- [x] Connect to random or fastest server from list
- [x] Select protocol, `tcp` or `udp` with -p flag
- [x] Filter by features
- [ ] Killswitch (probably linux only)
- [ ] Split tunneling (probably linux only)

## Todo / Bugs
- [x] MacOS Support
- [x] Fix daemon stuck deactivating sometimes while waiting for process to quit when it already has
- [x] Investigate why tcp doesn't work
- [x] Multiple openvpn services launched (saw this once)
- [ ] Clean up openvpn process if it wasn't cleanup up from last session
- [ ] Set up test suite
- [ ] Alert user in case traffic isn't actually being routed through tunnel
- [ ] Openvpn process not being killed if daemon is shut down before connection is established
- [ ] Add notify feature for desktop notifications
